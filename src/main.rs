mod lzss;

use std::{
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf, task::Context,
};

use byteorder::*;

#[derive(Debug)]
struct SubFile {
    offset: u32,
    size: u32,
    compressed: bool,
}

fn print_data(data: &[u8]) {
    for b in data {
        print!("{:02X}", b);
    }
    println!();
}

fn main() {
    let args = zigarg::Arguments::new();
    let slience = args.exist("-s");
    if !slience {
        println!("Star Force Archive Tool (Rust) v1.0 by SteveXMH (Original by Prof.9)");
    }
    if !args.has_args() {
        println!();
        println!("Usage:  sfarctool.exe <options>");
        println!("Options:");
        println!("        -i [path]       Specifies input path.");
        println!("        -o [path]       Specifies output path.");
        println!("        -x              Unpacks archive to folder. Requires -i and -o.");
        println!("        -p              Packs folder to archive. Requires -i and -o.");
        println!("        -eof            Indicates the archive has an EOF subfile entry. Requires -x or -p.");
        println!("        -c              Compress sub files if can be smaller. Requires -p.");
        println!("        --ignore-zero   Skip zero sized sub files when unpacking or add zero size sub files on missing index file when packing. Requires -x or -p.");
        println!("        -s              Slience mode. No output.");
        println!("        -v              Toggle verbose mode which will output a lot of message.");
        println!();
        println!("For option -p, subfiles in the input directory must be named as \"XXX.ext\" or \"name_XXX.ext\", \
                    where \"name\" is an arbitrary string not containing '.' or '_', \
                    \"XXX\" is the subfile number and \"ext\" is any extension \
                    (multiple extensions are allowed. \
                    Any files that do not adhere to this format will be skipped or be writen as zero size sub file when using --ignore-zero option.");
        return;
    }
    let input = args
        .get_value("-i")
        .cloned()
        .expect("Not selected an input path");
    let output = args
        .get_value("-o")
        .cloned()
        .expect("Not selected a output path");
    let unpack = args.exist("-x");
    let pack = args.exist("-p");
    let eof = args.exist("-eof");
    let compress = args.exist("-c");
    let verbose = args.exist("-v");
    let ignore_zero = args.exist("--ignore-zero");
    if (unpack && pack) || (!unpack && !pack) {
        println!("Error: Both set pack and unpack mode.");
        return;
    }
    if unpack {
        if verbose {
            println!("Unpacking archive");
        }
        let mut subfiles: Vec<SubFile> = Vec::new();
        let mut file = OpenOptions::new()
            .read(true)
            .open(&input)
            .expect("Can't open input archive");
        let file_size = file
            .metadata()
            .expect("Can't get metadata of the input archive")
            .len() as usize;
        let mut header_end = file_size;
        let mut pos = 0;
        let mut max_size = 0;
        while pos < header_end {
            let offset = file.read_u32::<LE>().expect("Can't read offset");
            let size = file.read_u32::<LE>().expect("Can't read size");
            pos += 8;
            let subfile = SubFile {
                offset: offset as _,
                size: (size & 0x7FFFFFFF) as _,
                compressed: (size & 0x80000000) != 0,
            };
            if verbose {
                if subfile.compressed {
                    println!(
                        "Entry {} at 0x{:08x}, size 0x{:08x}, compressed",
                        subfiles.len(),
                        subfile.offset,
                        subfile.size
                    );
                } else {
                    println!(
                        "Entry {} at 0x{:08x}, size 0x{:08x}",
                        subfiles.len(),
                        subfile.offset,
                        subfile.size
                    );
                }
            }
            subfiles.push(subfile);
            header_end = (offset as usize).min(header_end);
            max_size = (size as usize).max(max_size);
        }
        if pos != header_end
            || (eof
                && (subfiles.last().expect("").size > 0 || subfiles.last().expect("").compressed))
        {
            println!("Invalid archive file header.");
            return;
        }
        if eof {
            subfiles.pop();
        }
        let subfile_len = subfiles.len() - 1;
        let output_basename = {
            if let Some(sep) = input.as_str().rfind(['/', '\\']) {
                if let Some(dot) = input.as_str().rfind('.') {
                    input.as_str()[sep + 1..dot].to_owned()
                } else {
                    input.as_str()[sep + 1..].to_owned()
                }
            } else {
                input.to_owned()
            }
        };
        let max_size = subfiles.iter().map(|x| x.size).max().unwrap_or_default();
        if verbose {
            println!("Largest entry size: 0x{:08x}", max_size);
        }
        let mut buf = vec![0; max_size as _];
        let padding = (subfiles.len() - 1).to_string().len();
        let to_padded_string = |num: u32| -> String {
            let num = num.to_string();
            let mut padding = "0".to_string().repeat(padding - num.len());
            padding.push_str(num.as_str());
            padding
        };
        let output_basename = output_basename;
        std::fs::create_dir_all(&output).expect("Can't create output directory");
        for (i, subfile) in subfiles.into_iter().enumerate() {
            let subfile = subfile;
            if i == subfile_len
                && subfile.offset == file_size as _
                && subfile.size == 0xFFFF
                && !subfile.compressed
            {
                continue;
            }
            if ignore_zero && subfile.size == 0 {
                println!("Warning: Entry {} is empty, skipped.", i);
                continue;
            }
            let output_name = format!("{}_{}.bin", output_basename, to_padded_string(i as _));
            let mut output = std::path::PathBuf::from(&output);
            output.push(output_name);
            let mut output = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .open(output)
                .expect("Can't write subfile");
            file.seek(SeekFrom::Start(subfile.offset as _))
                .expect("Can't seek file");
            if subfile.compressed {
                if verbose {
                    println!("Decompressing entry {}", i);
                }
                let decompressed =
                    nintendo_lz::decompress(&mut file).expect("Can't decompress file");
                output
                    .write_all(&decompressed)
                    .expect("Can't write subfile");
            } else {
                if verbose {
                    println!("Unpacking entry {} with size {}", i, subfile.size);
                }
                file.read_exact(&mut buf[..subfile.size as usize])
                    .expect("Can't read file");
                output
                    .write_all(&buf[..subfile.size as usize])
                    .expect("Can't write subfile");
            }
        }
    } else if pack {
        // Parse files and sort them.
        let mut input_dir = std::fs::read_dir(&input).expect("Can't read input directory");
        let mut files = Vec::new();
        while let Some(Ok(entry)) = input_dir.next() {
            if entry.path().is_file() {
                let file_name = entry.file_name();
                let file_name = file_name.to_string_lossy();
                let dot = file_name.rfind('.');
                let underline = file_name.rfind('_');
                if let (Some(dot), Some(underline)) = (dot, underline) {
                    if let Ok(index) = file_name[underline + 1..dot].parse::<usize>() {
                        files.push((index, entry.path()))
                    }
                }
            }
        }
        files.sort_by(|a, b| a.0.cmp(&b.0));
        if !files.is_empty() {
            let max_index = files.last().and_then(|x| Some(x.0)).unwrap();
            let mut i = 0;
            if files.len() < max_index + 1 && ignore_zero {
                while files.len() < max_index + 1 {
                    let file = &files[i];
                    if i != file.0 {
                        files.insert(i, (i, PathBuf::default()));
                        if !slience {
                            println!("Warning: Missing file {}, using zero size file", i);
                        }
                    }
                    i += 1;
                }
            } else {
                println!(
                    "Incorrect subfile amount, expecting {} files but got {} subfiles",
                    max_index,
                    files.len()
                );
                println!("List of missing sub files:");
                for i in 0..max_index + 1 {
                    if files.iter().find(|x| x.0 == i).is_none() {
                        println!("Missing sub file {}", i);
                    }
                }
                println!("Tip: If it's not an error, use --ignore-zero to ignore missing files and write zero size sub file.");
                return;
            }
        }
        let header_size = if eof {
            (files.len() + 2) * 8
        } else {
            (files.len() + 1) * 8
        };
        let mut offset = header_size as u32;
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&output)
            .expect("Can't open output file");
        let mut file_size = header_size;
        for _ in 0..files.len() + 1 {
            file.write_u64::<LE>(0).expect("Can't fill header");
        }
        let mut buf = Vec::new();
        let files = files
            .into_iter()
            .map(|(i, entry)| {
                if entry == PathBuf::default() {
                    (false, vec![], 0)
                } else {
                    let mut subfile = OpenOptions::new()
                        .read(true)
                        .open(&entry)
                        .expect("Can't open subfile");
                    buf.clear();
                    subfile.read_to_end(&mut buf).expect("Can't read subfile");
                    let uncompressed_size = buf.len();
                    if compress {
                        if let Ok(compressed_data) = lzss::compress_arr(&buf) {
                            if uncompressed_size <= compressed_data.len() {
                                file_size += uncompressed_size;
                                (false, buf.to_owned(), uncompressed_size)
                            } else {
                                file_size += compressed_data.len();
                                (true, compressed_data, uncompressed_size)
                            }
                        } else {
                            (false, buf.to_owned(), uncompressed_size)
                        }
                    } else {
                        (false, buf.to_owned(), uncompressed_size)
                    }
                }
            })
            .collect::<Vec<_>>();
        file.seek(SeekFrom::Start(0))
            .expect("Can't seek file to start");
        for (compressed, data, uncompressed_size) in &files {
            if verbose {
                println!(
                    "Subfile {} bytes -> {} bytes",
                    data.len(),
                    uncompressed_size
                );
            }
            file.write_u32::<LE>(offset)
                .expect("Can't write file size at end of entries");
            file.write_u32::<LE>(
                (*uncompressed_size as u32 & 0x7FFFFFFF) | if *compressed { 0x80000000 } else { 0 },
            )
            .expect("Can't write file size at end of entries");
            offset += data.len() as u32;
        }
        if eof {
            file.write_u32::<LE>(file_size as _)
                .expect("Can't write file size at end of entries");
            file.write_u32::<LE>(0)
                .expect("Can't write file size at end of entries");
        }
        file.write_u32::<LE>(file_size as _)
            .expect("Can't write file size at end of entries");
        file.write_u32::<LE>(0xFFFF)
            .expect("Can't write file size at end of entries");
        for (_, data, _) in files {
            file.write_all(&data)
                .expect("Can't write subfile to archive")
        }
    }
}
