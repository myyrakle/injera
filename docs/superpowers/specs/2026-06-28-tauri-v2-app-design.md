# Tauri v2 App Design

## Goal

Rewrite `injera` as a Tauri v2 application that can be packaged as a desktop GUI app and prepared for mobile app builds, while preserving the existing Rust CLI.

## Current Context

`injera` is a Rust CLI and library for batch file renaming. It currently supports:

- Sequence rename: naturally sort files in a directory and rename them to padded numbers while preserving extensions.
- Regex rename: replace file names using a regular expression and replacement string.
- Collision prevention before rename execution.
- Progress logs through writer-based APIs.

The existing tests cover CLI argument parsing and core rename behavior.

## Chosen Approach

Use the repository as a Rust workspace with two surfaces:

- Keep the existing `injera` crate as the reusable core and CLI.
- Add a Tauri v2 desktop/mobile app under `src-tauri` with a Vite/TypeScript frontend.

This avoids throwing away tested rename logic. It also keeps terminal users unbroken while adding GUI packaging.

## Architecture

### Core Rust Library

The `renamer` module will expose preview-first APIs:

- `plan_sequence_rename(directory) -> RenamePlan`
- `plan_regex_rename(directory, pattern, replacement) -> RenamePlan`
- `apply_rename_plan(plan) -> RenameReport`

Existing CLI functions will call these APIs and continue writing the same progress logs. Planning and execution are separated so the GUI can show exactly what will happen before changing files.

### Tauri Backend

The Tauri crate will expose commands:

- `preview_sequence(directory: String)`
- `preview_regex(directory: String, pattern: String, replacement: String)`
- `apply_rename(plan: RenamePlan)`

Commands return serializable structs that the frontend can display. Errors are converted into clear strings.

### Frontend

The app opens directly into the rename workflow:

- Directory picker.
- Mode selector for Sequence or Regex.
- Regex pattern and replacement inputs when Regex is selected.
- Preview table with old name and new name.
- Apply button disabled until a valid preview exists.
- Status area for success and error messages.

The UI should feel like a utility app, not a marketing page: compact controls, readable table, restrained styling, and mobile-friendly layout.

### Mobile Notes

Tauri v2 mobile builds will be supported at the project configuration level. File access depends on Android/iOS platform permissions and Tauri dialog/path capabilities. The app will use Tauri-supported directory selection APIs where available, and keep the rename command interface platform-neutral.

## Error Handling

- Invalid regex returns a user-facing validation error.
- Duplicate target names prevent execution.
- Existing non-source targets prevent execution.
- I/O errors are surfaced in the status area and returned by commands.
- The GUI never applies changes without a successful preview.

## Testing

- Add Rust tests for preview plan structs and apply behavior.
- Preserve existing CLI tests.
- Add frontend build verification.
- Run `cargo test`, `cargo fmt --check`, and `npm run build`.

## Out Of Scope

- Undo history.
- Recursive rename.
- Custom numbering templates.
- Platform store submission.
- iOS signing or Android keystore generation.
