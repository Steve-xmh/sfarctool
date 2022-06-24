//! From [nintendo-lz](https://gitlab.com/DarkKirb/nintendo-lz) crate
use std::{
    error::Error,
    fmt::{Debug, Display},
    io::Read,
};

use byteorder::{LittleEndian, ReadBytesExt};

pub struct InvalidMagicNumberError;

impl Debug for InvalidMagicNumberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InvalidMagicNumberError")
    }
}

impl Display for InvalidMagicNumberError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InvalidMagicNumberError")
    }
}

impl Error for InvalidMagicNumberError {}

/// Decompresses an LZ10/LZ11 compressed file. It returns an error when:
///
/// - The file is not a valid LZ10/LZ11 file
/// - The file is truncated (More data was expected than present)
///
/// # Example
///
/// ```rust,ignore
/// let mut f = File::open("Archive.bin.cmp");
/// let mut decompressed = nintendo_lz::decompress(&mut f).unwrap();
/// ```
pub fn decompress(inp: &mut impl Read) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut length = inp.read_u32::<LittleEndian>()? as usize;
    let ver = match length & 0xFF {
        0x10 => Ok(0),
        0x11 => Ok(1),
        _ => Err(InvalidMagicNumberError),
    }?;
    length >>= 8;
    if length == 0 && ver == 1 {
        length = inp.read_u32::<LittleEndian>()? as usize;
    }
    let mut out: Vec<u8> = Vec::new();
    out.reserve(length);
    while out.len() < length {
        let byte = inp.read_u8()?;
        for bit_no in (0..8).rev() {
            if out.len() >= length {
                break;
            }
            if ((byte >> bit_no) & 1) == 0 {
                let data = inp.read_u8()?;
                out.push(data);
            } else {
                let lenmsb = inp.read_u8()? as usize;
                let lsb = inp.read_u8()? as usize;
                let mut length: usize = lenmsb >> 4;
                let mut disp: usize = ((lenmsb & 15) << 8) + lsb;
                if ver == 0 {
                    length += 3;
                } else if length > 1 {
                    length += 1;
                } else if length == 0 {
                    length = (lenmsb & 15) << 4;
                    length += lsb >> 4;
                    length += 0x11;
                    let msb = inp.read_u8()? as usize;
                    disp = ((lsb & 15) << 8) + msb;
                } else {
                    length = (lenmsb & 15) << 12;
                    length += lsb << 4;
                    let byte1 = inp.read_u8()? as usize;
                    let byte2 = inp.read_u8()? as usize;
                    length += byte1 >> 4;
                    length += 0x111;
                    disp = ((byte1 & 15) << 8) + byte2;
                }
                let start: usize = out.len() - disp - 1;

                for i in 0..length {
                    let val = out[start + i];
                    out.push(val);
                }
            }
        }
    }
    Ok(out)
}
