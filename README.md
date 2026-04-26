# injera

[English](README.md) | [한국어](README.ko.md)

`injera` is a convenience CLI tool for file management.

Currently supported features:

- Batch rename files to sequential names such as `00001` and `00002` after natural sorting
- Batch rename file names with a regular expression pattern and replacement

## Installation

You need the Rust toolchain installed.

```bash
cargo build --release
```

After the build finishes, the executable is created at:

```bash
./target/release/injera
```

You can also run it locally with `cargo run`.

```bash
cargo run -- <COMMAND>
```

## Usage

Show the full CLI help:

```bash
cargo run -- --help
```

Show help for rename commands:

```bash
cargo run -- rename --help
```

## Sequential Rename

Rename every regular file in a directory to a sequential name after natural sorting by the original file name.

```bash
cargo run -- rename sequence <DIR>
```

Example:

```bash
cargo run -- rename sequence ./photos
```

Rules:

- Directories are excluded from rename targets.
- Existing file extensions are preserved.
- Files without extensions use only the sequential number.
- The zero-padding width is calculated from the number of files.
- The minimum padding width is 5 digits.

For example, if `./photos` contains:

```text
apple.jpg
banana.txt
carrot
```

They are renamed as follows:

```text
00001.jpg
00002.txt
00003
```

If there are 100000 files, the width grows as needed, such as `000001`.

File names containing numbers are sorted in a human-friendly natural order.

```text
file-1.txt
file-2.txt
file-10.txt
```

These become `00001.txt`, `00002.txt`, and `00003.txt` in that order.

Progress logs are printed while the command runs.

```text
Scanning ./photos
Found 3 files
[1/3] apple.jpg -> 00001.jpg
[2/3] banana.txt -> 00002.txt
[3/3] carrot -> 00003
Done
```

## Regex Rename

Rename every regular file in a directory by applying a regular expression replacement to its file name.

```bash
cargo run -- rename regex <DIR> <PATTERN> <REPLACEMENT>
```

Example:

```bash
cargo run -- rename regex ./photos '^IMG_(\d+)\.(jpg|png)$' 'photo-$1.$2'
```

This renames files as follows:

```text
IMG_001.jpg -> photo-001.jpg
IMG_002.png -> photo-002.png
```

You can use capture groups such as `$1` and `$2` in the replacement.

Another example:

```bash
cargo run -- rename regex ./docs '-' '_'
```

This replaces every `-` in file names with `_`.

```text
daily-report-draft.txt -> daily_report_draft.txt
```

Regex rename also prints progress logs.

```text
Scanning ./photos
Found 2 files
[1/2] IMG_001.jpg -> photo-001.jpg
[2/2] IMG_002.png -> photo-002.png
Done
```

## Conflict Handling

If multiple files resolve to the same final name, the operation stops.

For example, this command fails because both `a.txt` and `b.txt` would become `same.txt`.

```bash
cargo run -- rename regex ./files '^[ab]\.txt$' 'same.txt'
```

The command also returns an error before renaming if a final target path is blocked by an existing item such as a directory.

## Development

Run tests:

```bash
cargo test
```

Check formatting:

```bash
cargo fmt --check
```
