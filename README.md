# exifsync

A small command-line tool that checks whether the file dates of your JPEG photos match the date stored in their EXIF data, and can correct the file dates if they don't.

## Why

Copying, restoring or syncing photos often resets a file's created and modified dates, so photos end up sorted by when they were copied rather than when they were taken. The capture date is usually still safe inside the EXIF data. `exifsync` finds the files where the two disagree and, if you ask it to, sets the file dates back to the EXIF date.

Only file system timestamps are changed. The contents of the photo are never modified.

## Build

Requires Rust 1.85 or newer (edition 2024).

```bash
cargo build --release
```

The binary is written to `target/release/exifsync` (`exifsync.exe` on Windows).

## Usage

```text
exifsync [OPTIONS] <DIR>
```

| Argument / option | Description |
| --- | --- |
| `<DIR>` | Directory to scan recursively |
| `--update` | Set the file's created/modified dates to the EXIF date instead of only listing discrepancies |
| `-h`, `--help` | Print help |
| `-V`, `--version` | Print version |

By default nothing is changed, so it is safe to run a check first:

```bash
exifsync "D:\Photos"
```

Then apply the corrections:

```bash
exifsync "D:\Photos" --update
```

### Example output

```text
D:\Photos\2004\09\25\a.jpg
    exif:     2004-09-25 11:28:37
    created:  2026-11-05 16:26:20  <-- differs
    modified: 2024-01-15 10:00:00  <-- differs
    updated

Scanned:    1
No EXIF:    0
In sync:    0
Mismatched: 1
Updated:    1
Errors:     0
```

Only mismatched files are listed, followed by the totals. The `updated` line and the `Updated` total appear only with `--update`.

## How it works

- **Files:** every `.jpg` and `.jpeg` file under the directory, in any letter case.
- **EXIF date:** `DateTimeOriginal`, then `DateTimeDigitized`, then `DateTime`, using the first one that is valid.
- **Time zone:** the matching EXIF offset tag is used if there is one. Otherwise the date is treated as local time on the machine running the tool.
- **Comparison:** whole seconds, because that is the resolution of EXIF dates.
- **No usable EXIF date:** the file is counted under `No EXIF` and left alone.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Finished without errors. Mismatches do not count as errors. |
| `1` | Finished, but at least one file or directory could not be processed |
| `2` | Invalid arguments, or `<DIR>` is not a directory |

## Platform notes

- **Windows and macOS:** `--update` sets both the created and the modified date.
- **Other platforms, such as Linux:** `--update` sets only the modified date. If the file system reports a creation date, that date can't be changed, so the file may keep being listed as mismatched after an update.

## Using it with archivejpg

If you also use [archivejpg](https://github.com/kreaton/archivejpg), run `exifsync --update` on the **source** folder before archiving. Updating only the archive makes archivejpg treat those photos as new on its next run and store them again. See the archivejpg README for details.
