use std::fs;
use std::path::{Path, PathBuf};

use injera::renamer::{
    apply_rename_plan, plan_regex_rename, plan_sequence_rename, rename_by_regex_with_writer,
    rename_by_sequence_with_writer,
};

#[test]
fn plan_sequence_rename_lists_natural_order_targets() {
    let dir = test_dir("plan_sequence");
    create_file_with_content(&dir, "file-10.txt", "ten");
    create_file_with_content(&dir, "file-2.txt", "two");
    create_file_with_content(&dir, "file-1.txt", "one");

    let plan = plan_sequence_rename(&dir).expect("sequence plan should succeed");

    assert_eq!(plan.directory, dir);
    assert_eq!(plan.entries.len(), 3);
    assert_eq!(plan.entries[0].source_name, "file-1.txt");
    assert_eq!(plan.entries[0].target_name, "00001.txt");
    assert_eq!(plan.entries[1].source_name, "file-2.txt");
    assert_eq!(plan.entries[1].target_name, "00002.txt");
    assert_eq!(plan.entries[2].source_name, "file-10.txt");
    assert_eq!(plan.entries[2].target_name, "00003.txt");
}

#[test]
fn plan_regex_rename_lists_replacements() {
    let dir = test_dir("plan_regex");
    create_file(&dir, "img-001.jpg");
    create_file(&dir, "note.txt");

    let plan = plan_regex_rename(&dir, r"^img-(\d+)\.(.+)$", "photo-$1.$2")
        .expect("regex plan should succeed");

    assert_eq!(plan.entries.len(), 2);
    assert_eq!(plan.entries[0].source_name, "img-001.jpg");
    assert_eq!(plan.entries[0].target_name, "photo-001.jpg");
    assert_eq!(plan.entries[1].source_name, "note.txt");
    assert_eq!(plan.entries[1].target_name, "note.txt");
}

#[test]
fn apply_rename_plan_changes_files() {
    let dir = test_dir("apply_plan");
    create_file_with_content(&dir, "file-2.txt", "two");
    create_file_with_content(&dir, "file-1.txt", "one");
    let plan = plan_sequence_rename(&dir).expect("sequence plan should succeed");

    let report = apply_rename_plan(&plan).expect("apply should succeed");

    assert_eq!(report.renamed_count, 2);
    assert_eq!(read_file(&dir, "00001.txt"), "one");
    assert_eq!(read_file(&dir, "00002.txt"), "two");
}

#[test]
fn sequence_renames_files_in_lexical_order_and_keeps_extensions() {
    let dir = test_dir("sequence");
    create_file(&dir, "banana.txt");
    create_file(&dir, "apple.jpg");
    create_file(&dir, "carrot");
    fs::create_dir(dir.join("nested")).expect("directory should be created");

    let mut output = std::io::sink();
    rename_by_sequence_with_writer(&dir, &mut output).expect("sequence rename should succeed");

    assert_eq!(file_names(&dir), ["00001.jpg", "00002.txt", "00003"]);
    assert!(dir.join("nested").is_dir());
}

#[test]
fn sequence_preserves_natural_file_name_order_when_numbering() {
    let dir = test_dir("sequence_natural_order");
    create_file_with_content(&dir, "file-10.txt", "ten");
    create_file_with_content(&dir, "file-2.txt", "two");
    create_file_with_content(&dir, "file-1.txt", "one");

    let mut output = std::io::sink();
    rename_by_sequence_with_writer(&dir, &mut output).expect("sequence rename should succeed");

    assert_eq!(read_file(&dir, "00001.txt"), "one");
    assert_eq!(read_file(&dir, "00002.txt"), "two");
    assert_eq!(read_file(&dir, "00003.txt"), "ten");
}

#[test]
fn sequence_writes_progress_logs() {
    let dir = test_dir("sequence_logs");
    create_file(&dir, "banana.txt");
    create_file(&dir, "apple.jpg");

    let mut output = Vec::new();
    rename_by_sequence_with_writer(&dir, &mut output).expect("sequence rename should succeed");

    let output = String::from_utf8(output).expect("log output should be UTF-8");
    assert!(output.contains("Scanning"));
    assert!(output.contains("Found 2 files"));
    assert!(output.contains("[1/2] apple.jpg -> 00001.jpg"));
    assert!(output.contains("[2/2] banana.txt -> 00002.txt"));
    assert!(output.contains("Done"));
}

#[test]
fn regex_renames_files_with_capture_replacements() {
    let dir = test_dir("regex");
    create_file(&dir, "img-001.jpg");
    create_file(&dir, "img-002.png");
    create_file(&dir, "note.txt");

    let mut output = std::io::sink();
    rename_by_regex_with_writer(&dir, r"^img-(\d+)\.(.+)$", "photo-$1.$2", &mut output)
        .expect("regex rename should succeed");

    assert_eq!(
        file_names(&dir),
        ["note.txt", "photo-001.jpg", "photo-002.png"]
    );
}

#[test]
fn regex_writes_progress_logs() {
    let dir = test_dir("regex_logs");
    create_file(&dir, "img-001.jpg");
    create_file(&dir, "note.txt");

    let mut output = Vec::new();
    rename_by_regex_with_writer(&dir, r"^img-(\d+)\.(.+)$", "photo-$1.$2", &mut output)
        .expect("regex rename should succeed");

    let output = String::from_utf8(output).expect("log output should be UTF-8");
    assert!(output.contains("Scanning"));
    assert!(output.contains("Found 2 files"));
    assert!(output.contains("[1/2] img-001.jpg -> photo-001.jpg"));
    assert!(output.contains("[2/2] note.txt -> note.txt"));
    assert!(output.contains("Done"));
}

#[test]
fn regex_replaces_all_matches_in_each_file_name() {
    let dir = test_dir("regex_global");
    create_file(&dir, "daily-report-draft.txt");

    let mut output = std::io::sink();
    rename_by_regex_with_writer(&dir, "-", "_", &mut output).expect("regex rename should succeed");

    assert_eq!(file_names(&dir), ["daily_report_draft.txt"]);
}

#[test]
fn regex_returns_error_when_multiple_files_resolve_to_same_name() {
    let dir = test_dir("regex_collision");
    create_file(&dir, "a.txt");
    create_file(&dir, "b.txt");

    let mut output = std::io::sink();
    let error = rename_by_regex_with_writer(&dir, r"^[ab]\.txt$", "same.txt", &mut output)
        .expect_err("duplicate targets should fail");

    assert_eq!(error.to_string(), "multiple files resolve to same.txt");
    assert_eq!(file_names(&dir), ["a.txt", "b.txt"]);
}

#[test]
fn sequence_returns_error_when_target_path_is_blocked() {
    let dir = test_dir("sequence_blocked");
    create_file(&dir, "apple.txt");
    fs::create_dir(dir.join("00001.txt")).expect("blocking directory should be created");

    let mut output = std::io::sink();
    let error =
        rename_by_sequence_with_writer(&dir, &mut output).expect_err("blocked target should fail");

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

fn create_file_with_content(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).expect("test file should be created");
}

fn read_file(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).expect("test file should be readable")
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
