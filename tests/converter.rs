use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use injera::converter::{ConvertOptions, TargetFormat, convert_archive, plan_archive_conversion};

#[test]
fn plan_names_the_clone_from_the_suffix() {
    let dir = test_dir("plan_names");
    let archive = dir.join("volume-1.cbz");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);

    let plan = plan_archive_conversion(std::slice::from_ref(&archive), &ConvertOptions::default())
        .expect("plan should succeed");

    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].source_name, "volume-1.cbz");
    assert_eq!(plan.entries[0].output_name, "volume-1-compressed.cbz");
    assert_eq!(plan.entries[0].output, dir.join("volume-1-compressed.cbz"));
    assert!(plan.entries[0].selected);
}

#[test]
fn plan_rejects_a_file_that_is_not_an_archive() {
    let dir = test_dir("plan_not_archive");
    let file = dir.join("notes.txt");
    fs::write(&file, b"text").expect("file should be created");

    let error = plan_archive_conversion(&[file], &ConvertOptions::default())
        .expect_err("a plain file should be rejected");

    assert_eq!(error.to_string(), "notes.txt is not a zip archive");
}

#[test]
fn plan_rejects_an_existing_output() {
    let dir = test_dir("plan_existing");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);
    fs::write(dir.join("volume-1-compressed.zip"), b"taken").expect("file should be created");

    let error = plan_archive_conversion(&[archive], &ConvertOptions::default())
        .expect_err("an existing output should be rejected");

    assert_eq!(error.to_string(), "volume-1-compressed.zip already exists");
}

#[test]
fn plan_rejects_an_empty_selection() {
    let error = plan_archive_conversion(&[], &ConvertOptions::default())
        .expect_err("an empty selection should be rejected");

    assert_eq!(error.to_string(), "no archives selected");
}

#[test]
fn plan_rejects_a_bad_quality() {
    let dir = test_dir("plan_quality");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);

    let options = ConvertOptions {
        quality: 0,
        ..ConvertOptions::default()
    };
    let error =
        plan_archive_conversion(&[archive], &options).expect_err("quality 0 should be rejected");

    assert_eq!(error.to_string(), "quality must be between 1 and 100");
}

#[test]
fn plan_rejects_a_suffix_with_a_path_separator() {
    let dir = test_dir("plan_suffix");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);

    let options = ConvertOptions {
        suffix: "../escaped".to_string(),
        ..ConvertOptions::default()
    };
    let error =
        plan_archive_conversion(&[archive], &options).expect_err("a separator should be rejected");

    assert_eq!(error.to_string(), "suffix must not contain path separators");
}

#[test]
fn convert_rewrites_images_as_jpeg_and_copies_the_rest() {
    let dir = test_dir("convert_jpeg");
    let archive = dir.join("volume-1.cbz");
    write_archive(
        &archive,
        &[
            ("info.txt", b"metadata".to_vec()),
            ("pages/page-1.png", png(64, 64)),
            ("pages/page-2.png", png(64, 64)),
            ("__MACOSX/pages/._page-1.png", b"fork".to_vec()),
        ],
    );

    let options = ConvertOptions {
        format: TargetFormat::Jpeg,
        quality: 60,
        ..ConvertOptions::default()
    };
    let plan = plan_archive_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_archive(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_converted, 2);
    assert_eq!(report.images_skipped, 0);
    assert_eq!(report.entries_copied, 1);
    assert!(report.output.exists());

    let names = archive_names(&report.output);
    assert_eq!(names, ["info.txt", "pages/page-1.jpg", "pages/page-2.jpg"]);
    assert_eq!(read_entry(&report.output, "info.txt"), b"metadata");
    assert_eq!(
        image::guess_format(&read_entry(&report.output, "pages/page-1.jpg")).expect("format"),
        image::ImageFormat::Jpeg
    );
}

#[test]
fn convert_keeps_the_original_format_by_default() {
    let dir = test_dir("convert_keep");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", png(32, 32))]);

    let options = ConvertOptions::default();
    let plan = plan_archive_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_archive(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_converted, 1);
    assert_eq!(archive_names(&report.output), ["page-1.png"]);
    assert_eq!(
        image::guess_format(&read_entry(&report.output, "page-1.png")).expect("format"),
        image::ImageFormat::Png
    );
}

#[test]
fn convert_leaves_the_source_untouched() {
    let dir = test_dir("convert_source");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", png(32, 32))]);
    let before = fs::read(&archive).expect("source should be readable");

    let options = ConvertOptions::default();
    let plan = plan_archive_conversion(std::slice::from_ref(&archive), &options)
        .expect("plan should succeed");
    convert_archive(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(
        fs::read(&archive).expect("source should be readable"),
        before
    );
}

#[test]
fn convert_copies_images_it_cannot_decode() {
    let dir = test_dir("convert_broken");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", b"not really a png".to_vec())]);

    let options = ConvertOptions::default();
    let plan = plan_archive_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_archive(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_converted, 0);
    assert_eq!(report.images_skipped, 1);
    assert_eq!(
        read_entry(&report.output, "page-1.png"),
        b"not really a png"
    );
}

#[test]
fn convert_shrinks_a_photographic_page_at_low_quality() {
    let dir = test_dir("convert_shrink");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", noisy_png(256, 256))]);

    let options = ConvertOptions {
        format: TargetFormat::Jpeg,
        quality: 30,
        ..ConvertOptions::default()
    };
    let plan = plan_archive_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_archive(&plan.entries[0], &options).expect("conversion should succeed");

    assert!(
        report.output_bytes < report.source_bytes,
        "expected the clone to shrink: {} -> {}",
        report.source_bytes,
        report.output_bytes
    );
}

fn test_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("injera_convert_{name}_{}", std::process::id()));

    if path.exists() {
        fs::remove_dir_all(&path).expect("old test directory should be removed");
    }

    fs::create_dir(&path).expect("test directory should be created");
    path.canonicalize()
        .expect("test directory should canonicalize")
}

/// A flat image, which PNG stores very efficiently.
fn png(width: u32, height: u32) -> Vec<u8> {
    encode_png(image::RgbImage::from_pixel(
        width,
        height,
        image::Rgb([90, 140, 200]),
    ))
}

/// Noise, so PNG cannot beat a low-quality JPEG.
fn noisy_png(width: u32, height: u32) -> Vec<u8> {
    let mut seed = 12345u32;
    let image = image::RgbImage::from_fn(width, height, |_, _| {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        image::Rgb([(seed >> 16) as u8, (seed >> 8) as u8, seed as u8])
    });

    encode_png(image)
}

fn encode_png(image: image::RgbImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    image
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .expect("png should encode");
    bytes
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

fn archive_names(path: &Path) -> Vec<String> {
    let file = fs::File::open(path).expect("archive should open");
    let archive = zip::ZipArchive::new(file).expect("archive should parse");
    archive.file_names().map(str::to_owned).collect()
}

fn read_entry(path: &Path, name: &str) -> Vec<u8> {
    let file = fs::File::open(path).expect("archive should open");
    let mut archive = zip::ZipArchive::new(file).expect("archive should parse");
    let mut entry = archive.by_name(name).expect("entry should exist");
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).expect("entry should read");
    bytes
}
