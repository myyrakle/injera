use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use injera::browser::{first_archive_image, list_directory};

#[test]
fn list_directory_sorts_entries_naturally() {
    let dir = test_dir("list_natural");
    create_file(&dir, "file-10.txt");
    create_file(&dir, "file-2.txt");
    create_file(&dir, "file-1.txt");
    fs::create_dir(dir.join("folder-10")).expect("directory should be created");
    fs::create_dir(dir.join("folder-2")).expect("directory should be created");

    let listing = list_directory(&dir).expect("listing should succeed");

    assert_eq!(
        names(
            &listing
                .files
                .iter()
                .map(|file| file.name.clone())
                .collect::<Vec<_>>()
        ),
        ["file-1.txt", "file-2.txt", "file-10.txt"]
    );
    assert_eq!(
        names(
            &listing
                .directories
                .iter()
                .map(|entry| entry.name.clone())
                .collect::<Vec<_>>()
        ),
        ["folder-2", "folder-10"]
    );
}

#[test]
fn list_directory_reports_the_parent_and_marks_archives() {
    let dir = test_dir("list_parent");
    create_file(&dir, "notes.txt");
    create_file(&dir, "volume.CBZ");
    create_file(&dir, "album.zip");

    let listing = list_directory(&dir).expect("listing should succeed");

    assert_eq!(listing.path, dir);
    assert_eq!(listing.parent, dir.parent().map(Path::to_path_buf));

    let archives = listing
        .files
        .iter()
        .filter(|file| file.is_archive)
        .map(|file| file.name.clone())
        .collect::<Vec<_>>();
    assert_eq!(archives, ["album.zip", "volume.CBZ"]);
}

#[test]
fn list_directory_reports_file_sizes() {
    let dir = test_dir("list_sizes");
    fs::write(dir.join("a.txt"), b"12345").expect("file should be created");

    let listing = list_directory(&dir).expect("listing should succeed");

    assert_eq!(listing.files[0].size, 5);
}

#[test]
fn list_directory_rejects_a_missing_path() {
    let dir = test_dir("list_missing");

    let error = list_directory(&dir.join("nope")).expect_err("missing path should fail");

    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
}

#[test]
fn first_archive_image_reads_the_first_image_in_natural_order() {
    let dir = test_dir("archive_first");
    let archive = dir.join("volume.cbz");
    write_archive(
        &archive,
        &[
            ("cover.txt", b"not an image".to_vec()),
            ("page-10.png", b"ten".to_vec()),
            ("page-2.png", b"two".to_vec()),
        ],
    );

    let preview = first_archive_image(&archive)
        .expect("archive should open")
        .expect("archive should hold an image");

    assert_eq!(preview.entry_name, "page-2.png");
    assert_eq!(preview.data_uri, "data:image/png;base64,dHdv");
}

#[test]
fn first_archive_image_skips_macos_metadata_entries() {
    let dir = test_dir("archive_macos");
    let archive = dir.join("volume.zip");
    write_archive(
        &archive,
        &[
            ("__MACOSX/page-1.jpg", b"fork".to_vec()),
            ("pages/._page-1.jpg", b"fork".to_vec()),
            ("pages/page-1.jpg", b"real".to_vec()),
        ],
    );

    let preview = first_archive_image(&archive)
        .expect("archive should open")
        .expect("archive should hold an image");

    assert_eq!(preview.entry_name, "pages/page-1.jpg");
    assert_eq!(preview.data_uri, "data:image/jpeg;base64,cmVhbA==");
}

#[test]
fn first_archive_image_returns_none_when_no_image_exists() {
    let dir = test_dir("archive_no_image");
    let archive = dir.join("notes.zip");
    write_archive(&archive, &[("readme.txt", b"text".to_vec())]);

    let preview = first_archive_image(&archive).expect("archive should open");

    assert!(preview.is_none());
}

#[test]
fn first_archive_image_reports_a_broken_archive() {
    let dir = test_dir("archive_broken");
    let archive = dir.join("broken.zip");
    fs::write(&archive, b"definitely not a zip").expect("file should be created");

    let error = first_archive_image(&archive).expect_err("broken archive should fail");

    assert!(!error.to_string().is_empty());
}

#[cfg(unix)]
#[test]
fn list_directory_explains_a_permission_error() {
    use std::os::unix::fs::PermissionsExt;

    let dir = test_dir("list_denied");
    let locked = dir.join("locked");
    fs::create_dir(&locked).expect("directory should be created");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000))
        .expect("permissions should be set");

    let error = list_directory(&locked).expect_err("an unreadable directory should fail");

    // Restore before asserting, so a failure does not leave the tree unremovable.
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755))
        .expect("permissions should be restored");

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        error.to_string().contains("Grant this app access"),
        "unhelpful message: {error}"
    );
}

fn test_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("injera_browser_{name}_{}", std::process::id()));

    if path.exists() {
        fs::remove_dir_all(&path).expect("old test directory should be removed");
    }

    fs::create_dir(&path).expect("test directory should be created");
    path.canonicalize()
        .expect("test directory should canonicalize")
}

fn create_file(dir: &Path, name: &str) {
    fs::write(dir.join(name), b"test").expect("test file should be created");
}

fn write_archive(path: &Path, entries: &[(&str, Vec<u8>)]) {
    let file = fs::File::create(path).expect("archive should be created");
    let mut writer = zip::ZipWriter::new(file);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for (name, bytes) in entries {
        writer
            .start_file(*name, options)
            .expect("entry should start");
        writer.write_all(bytes).expect("entry should be written");
    }

    writer.finish().expect("archive should finish");
}

fn names(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}
