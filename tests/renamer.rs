use std::fs;
use std::path::{Path, PathBuf};

use rust_template::renamer::{rename_by_regex, rename_by_sequence};

#[test]
fn sequence_renames_files_in_lexical_order_and_keeps_extensions() {
    let dir = test_dir("sequence");
    create_file(&dir, "banana.txt");
    create_file(&dir, "apple.jpg");
    create_file(&dir, "carrot");
    fs::create_dir(dir.join("nested")).expect("directory should be created");

    rename_by_sequence(&dir).expect("sequence rename should succeed");

    assert_eq!(file_names(&dir), ["00001.jpg", "00002.txt", "00003"]);
    assert!(dir.join("nested").is_dir());
}

#[test]
fn regex_renames_files_with_capture_replacements() {
    let dir = test_dir("regex");
    create_file(&dir, "img-001.jpg");
    create_file(&dir, "img-002.png");
    create_file(&dir, "note.txt");

    rename_by_regex(&dir, r"^img-(\d+)\.(.+)$", "photo-$1.$2")
        .expect("regex rename should succeed");

    assert_eq!(
        file_names(&dir),
        ["note.txt", "photo-001.jpg", "photo-002.png"]
    );
}

#[test]
fn regex_replaces_all_matches_in_each_file_name() {
    let dir = test_dir("regex_global");
    create_file(&dir, "daily-report-draft.txt");

    rename_by_regex(&dir, "-", "_").expect("regex rename should succeed");

    assert_eq!(file_names(&dir), ["daily_report_draft.txt"]);
}

#[test]
fn regex_returns_error_when_multiple_files_resolve_to_same_name() {
    let dir = test_dir("regex_collision");
    create_file(&dir, "a.txt");
    create_file(&dir, "b.txt");

    let error = rename_by_regex(&dir, r"^[ab]\.txt$", "same.txt")
        .expect_err("duplicate targets should fail");

    assert_eq!(error.to_string(), "multiple files resolve to same.txt");
    assert_eq!(file_names(&dir), ["a.txt", "b.txt"]);
}

#[test]
fn sequence_returns_error_when_target_path_is_blocked() {
    let dir = test_dir("sequence_blocked");
    create_file(&dir, "apple.txt");
    fs::create_dir(dir.join("00001.txt")).expect("blocking directory should be created");

    let error = rename_by_sequence(&dir).expect_err("blocked target should fail");

    assert_eq!(error.to_string(), "target already exists: 00001.txt");
    assert_eq!(file_names(&dir), ["apple.txt"]);
    assert!(dir.join("00001.txt").is_dir());
}

fn test_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "rust_template_rename_{name}_{}",
        std::process::id()
    ));

    if path.exists() {
        fs::remove_dir_all(&path).expect("old test directory should be removed");
    }

    fs::create_dir(&path).expect("test directory should be created");
    path
}

fn create_file(dir: &Path, name: &str) {
    fs::write(dir.join(name), b"test").expect("test file should be created");
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names = fs::read_dir(dir)
        .expect("directory should be readable")
        .filter_map(|entry| {
            let entry = entry.expect("entry should be readable");
            entry
                .file_type()
                .expect("file type should be readable")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}
