# injera

[English](README.md) | [한국어](README.ko.md)

`injera` is a Tauri v2 app for batch file renaming on desktop and mobile.

## Features

- Preview every rename before applying changes.
- Rename files to natural-sort sequence names such as `00001.jpg`.
- Rename files with a regular expression pattern and replacement.
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

## iOS Build

iOS requires macOS and Xcode. Use the Tauri iOS commands from a macOS environment with the iOS toolchain installed.
