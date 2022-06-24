//! LZSS compression from https://github.com/magical/nlzss/blob/master/compress.py

use std::{
    collections::HashMap,
    error::Error,
    hash::Hash,
    io::{Cursor, Write},
    ops::Index,
};

use byteorder::*;
use itertools::Itertools;

#[derive(Debug)]
struct DefaultMap<K, V>(HashMap<K, V>);

impl<K, V> DefaultMap<K, V>
where
    K: Eq + Hash + Clone,
    V: Default + Clone,
{
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn get_mut(&mut self, k: &K) -> &mut V {
        if self.0.contains_key(k) {
            self.0.get_mut(k).unwrap()
        } else {
            self.0.insert(k.clone(), Default::default());
            self.0.get_mut(k).unwrap()
        }
    }
    pub fn get(&mut self, k: &K) -> V {
        if self.0.contains_key(k) {
            self.0.get(k).cloned().unwrap()
        } else {
            Default::default()
        }
    }
}

#[derive(Debug)]
pub struct CompressWindow<'a, const LEN: u32, const MIN: u32, const MAX: u32> {
    pub(self) input: &'a [u8],
    pub(self) hash: DefaultMap<u32, Vec<u32>>,
    pub(self) full: bool,
    pub(self) disp_min: u32,
    pub(self) disp_start: u32,
    pub(self) start: u32,
    pub(self) stop: u32,
    pub(self) index: u32,
}

pub type NLZ10Window<'a> = CompressWindow<'a, 4096, 3, { 3 + 0xF }>;
// pub type NLZ11Window<'a> = CompressWindow<'a, 4096, 3, { 0x111 + 0xFFFF }>;

impl<'a, const LEN: u32, const MIN: u32, const MAX: u32> CompressWindow<'a, LEN, MIN, MAX> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            hash: DefaultMap::new(),
            full: false,
            disp_min: 2,
            disp_start: 1,
            start: 0,
            stop: 0,
            index: 0,
        }
    }

    fn input_len(&self) -> u32 {
        self.input.len() as _
    }

    fn next(&mut self) {
        if self.index < self.disp_start - 1 {
            self.index += 1;
            return;
        }
        if self.full {
            let olditem = self.input[self.start as usize] as _;
            debug_assert!(self.hash.get(&olditem).first() == Some(&self.start));
            self.hash.get_mut(&olditem).remove(0);
        }
        let item = self.input[self.stop as usize] as u32;
        self.hash.get_mut(&item).push(self.stop);
        self.stop += 1;
        self.index += 1;

        if self.full {
            self.start += 1;
        } else if LEN <= self.stop {
            self.full = true;
        }
    }

    fn advance(&mut self, n: u32) {
        for _ in 0..n {
            self.next();
        }
    }

    fn search(&mut self) -> Option<(u32, i32)> {
        let mut counts = vec![];
        let input_byte = self.input[self.index as usize] as _;
        let indices = self.hash.get(&input_byte);
        for i in indices.iter() {
            let matchlen = self.match_data(*i, self.index);
            if matchlen >= MIN {
                let disp = self.index as i32 - *i as i32;
                // debug_assert!(self.index as i32 - disp >= 0);
                // debug_assert!(self.disp_min as i32 <= disp);
                // debug_assert!(disp <= (LEN + self.disp_min) as i32);
                if self.disp_min as i32 <= disp {
                    counts.push((matchlen, -disp));
                    if matchlen >= MAX {
                        return counts.last().cloned();
                    }
                }
            }
        }
        if !counts.is_empty() {
            let mut result = (0, 0);
            let mut t = 0;
            for count in counts {
                if t < count.0 {
                    t = count.0;
                    result = count;
                }
            }
            return Some(result);
        }
        None
    }

    fn match_data(&self, start: u32, bufstart: u32) -> u32 {
        let size = self.index - start;
        if size == 0 {
            return 0;
        }
        let mut matchlen = 0;
        let it = 0..(self.input.len() as u32 - bufstart).min(MAX);
        for i in it {
            if self.input[(start + (i % LEN)) as usize] == self.input[(bufstart + i) as usize] {
                matchlen += 1;
            } else {
                break;
            }
        }
        matchlen
    }
}

impl<const LEN: u32, const MIN: u32, const MAX: u32> Index<usize>
    for CompressWindow<'_, LEN, MIN, MAX>
{
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.input[index]
    }
}

#[derive(Debug)]
struct Compressor<'a, const LEN: u32, const MIN: u32, const MAX: u32>(
    CompressWindow<'a, LEN, MIN, MAX>,
    u32,
);

impl<'a, const LEN: u32, const MIN: u32, const MAX: u32> Compressor<'a, LEN, MIN, MAX> {
    pub fn new(window: CompressWindow<'a, LEN, MIN, MAX>) -> Self {
        Self(window, 0)
    }
}

#[derive(Debug)]
enum CompressChunkType {
    Replace(u32, i32),
    Data(u8),
}

impl<const LEN: u32, const MIN: u32, const MAX: u32> Iterator for Compressor<'_, LEN, MIN, MAX> {
    type Item = CompressChunkType;

    fn next(&mut self) -> Option<Self::Item> {
        if self.1 >= self.0.input_len() {
            return None;
        }
        if let Some(matched) = self.0.search() {
            self.0.advance(matched.0);
            self.1 += matched.0;
            // assert_ne!(matched.1, -283);
            Some(CompressChunkType::Replace(matched.0, matched.1))
        } else {
            let i = self.1;
            self.0.next();
            self.1 += 1;
            Some(CompressChunkType::Data(self.0[i as _]))
        }
    }
}

pub fn compress_nlz10(input: &[u8], output: &mut impl Write) -> Result<(), Box<dyn Error>> {
    output.write_u32::<LE>(((input.len() as u32) << 8) + 0x10)?;
    let mut length = 0;
    let window = NLZ10Window::new(input);
    for c in &Compressor::new(window).chunks(8) {
        let c = c.collect_vec();
        let mut flag = 0u8;
        let mut flagit = c
            .iter()
            .map(|x| matches!(x, CompressChunkType::Replace(_, _)));
        for _ in 0..8 {
            flag <<= 1;
            if let Some(c) = flagit.next() {
                if c {
                    flag |= 1;
                }
            }
        }
        output.write_u8(flag)?;
        length += 1;

        for c in c {
            match c {
                CompressChunkType::Replace(mut count, disp) => {
                    count -= 3;
                    let disp = (-disp) - 1;
                    assert!(0 <= disp);
                    assert!(disp < 4096);
                    let sh = (((count << 12) | disp.unsigned_abs()) & 0xFFFF) as u16;
                    output.write_u16::<BE>(sh)?;
                    length += 2;
                }
                CompressChunkType::Data(data) => {
                    output.write_u8(data)?;
                    length += 1;
                }
            }
        }
    }
    let padding = if length % 4 == 0 { 0 } else { 4 - length % 4 };
    for _ in 0..padding {
        output.write_u8(0xFF)?;
    }

    Ok(())
}

// pub fn compress_nlz11(input: &[u8], output: &mut impl Write) -> Result<(), Box<dyn Error>> {
//     output.write_u32::<LE>(((input.len() as u32) << 8) + 0x11)?;
//     let mut length = 0;

//     let window = NLZ11Window::new(input);
//     for c in &Compressor::new(window).chunks(8) {
//         let c = c.collect_vec();
//         let mut flag = 0u8;
//         let mut flagit = c
//             .iter()
//             .map(|x| matches!(x, CompressChunkType::Replace(_, _)));
//         for _ in 0..8 {
//             flag <<= 1;
//             if let Some(c) = flagit.next() {
//                 if c {
//                     flag |= 1;
//                 }
//             }
//         }
//         output.write_u8(flag)?;
//         length += 1;
//         for c in c {
//             match c {
//                 CompressChunkType::Replace(mut count, disp) => {
//                     let disp = (-disp) - 1;
//                     debug_assert!((0..=0xFFF).contains(&disp));
//                     if count <= 1 + 0xF {
//                         count -= 1;
//                         debug_assert!((2..=0xF).contains(&count));
//                         let sh = ((count << 12) & 0xFFFF) as u16 | (disp.abs() & 0xFFFF) as u16;
//                         output.write_u16::<BE>(sh)?;
//                         length += 2;
//                     } else if count <= 0x11 + 0xFF {
//                         count -= 0x11;
//                         debug_assert!((0..=0xFF).contains(&count));
//                         let b = (count >> 4 & 0xFF) as u8;
//                         let sh = ((count & 0xF) << 12) as u16 | disp.abs() as u16;
//                         output.write_u8(b)?;
//                         output.write_u16::<BE>(sh)?;
//                         length += 3;
//                     } else if count <= 0x111 + 0xFFFF {
//                         count -= 0x111;
//                         debug_assert!((0..=0xFFFF).contains(&count));
//                         let l = (1 << 28) | (count << 12) | disp.abs() as u32;
//                         output.write_u32::<BE>(l as _)?;
//                         length += 4;
//                     } else {
//                         panic!();
//                     }
//                 }
//                 CompressChunkType::Data(data) => {
//                     output.write_u8(data)?;
//                     length += 1;
//                 }
//             }
//         }
//     }
//     let padding = if length % 4 == 0 { 0 } else { 4 - length % 4 };
//     for _ in 0..padding {
//         output.write_u8(0xFF)?;
//     }

//     Ok(())
// }

pub fn compress_arr(input: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut result = Cursor::new(Vec::new());
    compress_nlz10(input, &mut result)?;
    Ok(result.into_inner())
}
