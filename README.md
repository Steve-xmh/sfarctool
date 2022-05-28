# sfarctool

A tool can unpack/pack game archive file from MegaMan Star Force (Ryuusei No Rockman) series.

This's a fork of [Prof.9's work](https://github.com/Prof9/SFArcTool), but also supported compression on packing.

## Usage

```bash
> sfarctool.exe
Star Force Archive Tool (Rust) v1.0 by SteveXMH (Original by Prof.9)

Usage:  sfarctool.exe <options>
Options:
        -i [path]       Specifies input path.
        -o [path]       Specifies output path.
        -x              Unpacks archive to folder. Requires -i and -o.
        -p              Packs folder to archive. Requires -i and -o.
        -eof            Indicates the archive has an EOF subfile entry. Requires -x or -p.
        -c              Compress sub files if can be smaller. Requires -p.
        --ignore-zero   Skip zero sized sub files when unpacking or add zero size sub files on missing index file when packing. Requires -x or -p.
        -s              Slience mode. No output.
        -v              Toggle verbose mode which will output a lot of message.

For option -p, subfiles in the input directory must be named as "XXX.ext" or "name_XXX.ext", where "name" is an arbitrary string not containing '.' or '_', "XXX" is the subfile number and "ext" is any extension (multiple extensions are allowed. Any files that do not adhere to this format will be skipped or be writen as zero size sub file when using --ignore-zero option.
```
