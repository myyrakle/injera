# injera

[English](README.md) | [한국어](README.ko.md)

`injera` is a Tauri v2 app for batch file renaming on desktop and mobile.

## Features

- Browse folders in the app, without depending on a native folder dialog.
- Pick exactly the files to rename with multi-select, or take the whole folder at once.
- See the first image inside each `.zip` or `.cbz` as a thumbnail, so archives are recognisable.
- Clone a `.zip`/`.cbz` with its images re-encoded, to shrink an archive without touching the original.
- Reopen the folder you were last browsing.
- Preview every rename before applying changes.
- Rename files to natural-sort sequence names such as `00001.jpg`.
- Choose a prefix, a start number, and a zero padding width for sequence names.
- Rename files with a regular expression pattern and replacement.
- Deselect individual files in the preview to leave them untouched.
- Prevent duplicate or blocked target names before execution.
- Build desktop packages and Android mobile artifacts from the same app code.

## Development

Install JavaScript dependencies:

```bash
npm install
```

Run the desktop app in development mode:

```bash
npm run tauri:dev
```

Run checks:

```bash
cargo fmt --check
cargo test --workspace
npm run build
```

## Browsing And Selecting

The app opens on your home folder and lists that folder's sub-folders and files in natural order, so
`scan-2.jpg` comes before `scan-10.jpg`.

- **Up** and **Home** move between folders; clicking a folder row enters it.
- **Choose** opens the native folder dialog on desktop as a shortcut.
- Tick the files to rename, or use **Select all**. Only the ticked files are renamed, and the
  sequence numbering follows their natural order.
- Changing folders clears the current selection and preview.

Files ending in `.zip` or `.cbz` are read for their first image, in natural order inside the archive,
and it is shown as a thumbnail. Archives holding no image, or whose first image is larger than 12 MB,
fall back to a plain placeholder. Thumbnails load only as rows scroll into view.

## Converting Archives

The **Convert** mode writes a compressed copy of each selected archive and never modifies the
original. `volume-1.cbz` becomes `volume-1-compressed.cbz` beside it.

| Option | Default | Effect |
| --- | --- | --- |
| Image format | Keep original | `Keep` re-encodes each image in its own format. `JPEG`, `PNG`, and `WebP` convert every image, and the entry extension changes to match. |
| Quality | `80` | JPEG quality, 1-100. PNG and WebP output is lossless, so the field is disabled for them. |
| Suffix | `-compressed` | Appended to the file stem to name the clone. Path separators are rejected. |

Non-image entries are copied through byte for byte, and an image that cannot be decoded is copied
unchanged rather than failing the archive. Entries above 64 MB are copied without decoding. The run
stops if a clone name already exists, so nothing is overwritten.

## Sequence Options

Sequence mode builds each name from a prefix, a number, and the original extension.

| Option | Default | Effect |
| --- | --- | --- |
| Prefix | empty | Text placed before the number, such as `photo-`. Path separators are rejected. |
| Start number | `1` | Number given to the first file in natural-sort order. |
| Padding | auto | Zero padding width. Auto uses at least 5 digits, and grows so the largest number fits. |

For example, a prefix of `photo-`, a start number of `8`, and a padding of `3` renames the first
file to `photo-008.jpg`.

## Selecting Files

Every previewed file is selected by default. Clear a row's checkbox to keep that file's current
name; the rename runs only on the selected rows. A file left unselected still occupies its name, so
the app reports an error instead of overwriting it.

## Desktop Build

Build desktop packages:

```bash
npm run tauri:build
```

On rolling Linux distributions where AppImage bundling fails while stripping newer ELF sections, use:

```bash
npm run tauri:build:linux
```

Linux packages are written under:

```text
target/release/bundle/
```

## Android Build

Initialize the Android project if it has not been generated yet:

```bash
npm run tauri:android:init
```

Build Android artifacts:

```bash
npm run tauri:android:build
```

The unsigned APK and AAB are written under:

```text
src-tauri/gen/android/app/build/outputs/
```

### Android Limitation

`tauri-plugin-dialog` returns `FolderPickerNotImplemented` for directory dialogs on Android and iOS,
so the **Choose** button does not work there. The in-app browser is the way in on mobile. Android
scoped storage still governs which folders the app may read, so folders outside the app's own storage
may list as unreadable until the platform grants access.

## iOS Build

iOS requires macOS and Xcode. Use the Tauri iOS commands from a macOS environment with the iOS toolchain installed.
