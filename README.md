# injera

[English](README.md) | [한국어](README.ko.md)

`injera` is a Tauri v2 app for batch file renaming on desktop and mobile.

## Features

- Browse folders in the app, without depending on a native folder dialog.
- Pick exactly the files to rename with multi-select, or take the whole folder at once.
- See the first image inside each `.zip` or `.cbz` as a thumbnail, so archives are recognisable.
- Re-encode images, either on their own or inside a `.zip`/`.cbz`, writing a new file and leaving the original alone.
- Reopen the folder you were last browsing.
- Preview every rename before applying changes.
- Rename files to natural-sort sequence names such as `00001.jpg`.
- Choose a prefix, a start number, and a zero padding width for sequence names.
- Rename files with a regular expression pattern and replacement.
- Deselect individual files in the preview to leave them untouched.
- Prevent duplicate or blocked target names before execution.
- Build desktop packages and Android mobile artifacts from the same app code.

## Development

`make` lists every target. The npm scripts underneath still work if you prefer them.

```bash
make          # list the targets
make dev      # run the desktop app with hot reload
make check    # formatting, clippy, tests, and the frontend build, as CI runs them
```

`make lint` pins the same toolchain as the lint workflow, so a green `make check` means a green CI.

## Browsing And Selecting

The app opens on your home folder and lists that folder's sub-folders and files in natural order, so
`scan-2.jpg` comes before `scan-10.jpg`.

- **Rename or convert** in the toolbar opens the settings over the list, so the action button is
  always one click away instead of below however many files the folder holds.
- **Up** and **Home** move between folders; clicking a folder row enters it.
- **Choose** opens the native folder dialog on desktop as a shortcut.
- Tick the files to rename, or use **Select all**. Only the ticked files are renamed, and the
  sequence numbering follows their natural order.
- Changing folders clears the current selection and preview.

Files ending in `.zip` or `.cbz` are read for their first image, in natural order inside the archive,
and it is shown as a thumbnail. Archives holding no image, or whose first image is larger than 12 MB,
fall back to a plain placeholder. Thumbnails load only as rows scroll into view.

## Converting Images

The **Convert** mode writes a converted copy of everything selected and never modifies the original.
It takes both kinds of input:

- An **archive** (`.zip`, `.cbz`) is cloned with every image inside it re-encoded.
  `volume-1.cbz` becomes `volume-1-compressed.cbz`.
- An **image file** is re-encoded on its own, taking the extension of the chosen format.
  `cover.png` becomes `cover-compressed.jpg` when the format is JPEG.

Both can be selected together. Anything else in the selection is left out.

`WebP` compresses scanned pages far harder than `JPEG` does. On a ten page scan of 24.6 MB, `JPEG`
at quality 75 gave 20.4 MB and `WebP` at the same quality gave 11.7 MB. Quality 100 asks for
lossless WebP, which stores every pixel and will be *larger* than a lossy source: 39.0 MB for the
same scan. Leave it below 100 unless lossless is what you want.

The plan follows from what is ticked and what the settings say, and updates itself as either
changes, so there is no preview step to press. An output name that is already taken, or two sources
that resolve to the same name, mark those rows instead of stopping the batch; the rest still run.

| Option | Default | Effect |
| --- | --- | --- |
| Image format | Keep original | `Keep` re-encodes each image in its own format. `JPEG`, `PNG`, and `WebP` convert every image, and the entry extension changes to match. |
| Quality | `80` | Quality for the lossy formats, 1-100. WebP treats 100 as lossless. PNG is always lossless, so the field is disabled for it. |
| Suffix | `-compressed` | Appended to the file stem to name the clone. Path separators are rejected. |

Images are converted across all cores, a batch at a time, so a large archive never has to fit in
memory. Progress is reported per image while a run is going.

Because the sources are left alone, the selection survives a run: change the quality or the suffix
and convert the same files again. The names just written show up as taken, so nothing is
overwritten by accident.

When the format is left at `Keep`, a re-encode that came out larger is thrown away and the original
bytes are kept, so shrinking an archive can never grow it. Asking for a specific format always
converts, since that is an explicit instruction.

Inside an archive, non-image entries are copied through byte for byte, an image that cannot be
decoded is copied unchanged rather than failing the archive, and entries above 64 MB are copied
without decoding. A standalone image that cannot be decoded is reported instead. The run stops if an
output name already exists or if two sources resolve to the same name, so nothing is overwritten.

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

`ANDROID_HOME` and `NDK_HOME` must be set; the Android targets check for them up front rather than
failing deep inside Gradle.

Initialize the Android project if it has not been generated yet:

```bash
make android-init
```

Build Android artifacts:

```bash
make android
make android-artifacts   # list what came out
```

Gradle's release APK is unsigned, and Android will not install an unsigned package. To try it on a
device, sign it with the local debug key:

```bash
make android-sign      # writes injera-universal-debugsigned.apk beside the unsigned one
make android-install   # signs, then adb installs it
```

A build for distribution needs a real keystore, not the debug key.

The unsigned APK and AAB are written under:

```text
src-tauri/gen/android/app/build/outputs/
```

### Android Storage Access

An Android app's own storage holds none of the user's archives, and reading anything else needs
permission. Without it every listing fails with `os error 13`, so the app asks on first launch:

- **API 30 and up** need *All files access*, which is a Settings screen rather than a dialog. The app
  opens it once per launch; granting it there and returning makes the whole device readable.
- **API 29 and below** get the ordinary read permission dialog.

The browser starts at `/storage/emulated/0` on Android rather than the app sandbox.

Android does not report a missing grant as an error: it filters the contents out and returns an empty
directory, so a folder full of archives simply looks empty. The app therefore checks its access by
writing a probe file into shared storage and removing it, and shows a banner over the file list when
that fails, rather than leaving you to guess why every folder is empty.

*All files access* is restricted on the Play Store. It suits a sideloaded build; a store release
would have to move to the Storage Access Framework instead.

`tauri-plugin-dialog` returns `FolderPickerNotImplemented` for directory dialogs on Android and iOS,
so the **Choose** button does not work there. The in-app browser is the way in on mobile.

## iOS Build

iOS requires macOS and Xcode. Use the Tauri iOS commands from a macOS environment with the iOS toolchain installed.
