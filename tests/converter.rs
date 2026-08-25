use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use injera::converter::{
    ConvertKind, ConvertOptions, TargetFormat, convert_entry, convert_entry_with_progress,
    plan_conversion,
};

#[test]
fn plan_names_the_clone_from_the_suffix() {
    let dir = test_dir("plan_names");
    let archive = dir.join("volume-1.cbz");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);

    let plan = plan_conversion(std::slice::from_ref(&archive), &ConvertOptions::default())
        .expect("plan should succeed");

    assert_eq!(plan.entries.len(), 1);
    assert_eq!(plan.entries[0].source_name, "volume-1.cbz");
    assert_eq!(plan.entries[0].output_name, "volume-1-compressed.cbz");
    assert_eq!(plan.entries[0].output, dir.join("volume-1-compressed.cbz"));
    assert_eq!(plan.entries[0].kind, ConvertKind::Archive);
    assert!(plan.entries[0].selected);
}

#[test]
fn plan_rejects_a_file_that_is_neither_an_archive_nor_an_image() {
    let dir = test_dir("plan_not_archive");
    let file = dir.join("notes.txt");
    fs::write(&file, b"text").expect("file should be created");

    let error = plan_conversion(&[file], &ConvertOptions::default())
        .expect_err("a plain file should be rejected");

    assert_eq!(error.to_string(), "notes.txt is not an archive or an image");
}

#[test]
fn plan_rejects_an_existing_output() {
    let dir = test_dir("plan_existing");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);
    fs::write(dir.join("volume-1-compressed.zip"), b"taken").expect("file should be created");

    let error = plan_conversion(&[archive], &ConvertOptions::default())
        .expect_err("an existing output should be rejected");

    assert_eq!(error.to_string(), "volume-1-compressed.zip already exists");
}

#[test]
fn plan_rejects_an_empty_selection() {
    let error = plan_conversion(&[], &ConvertOptions::default())
        .expect_err("an empty selection should be rejected");

    assert_eq!(error.to_string(), "nothing selected");
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
    let error = plan_conversion(&[archive], &options).expect_err("quality 0 should be rejected");

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
    let error = plan_conversion(&[archive], &options).expect_err("a separator should be rejected");

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
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

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
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_converted + report.images_kept, 1);
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
    let plan =
        plan_conversion(std::slice::from_ref(&archive), &options).expect("plan should succeed");
    convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

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
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

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
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

    assert!(
        report.output_bytes < report.source_bytes,
        "expected the clone to shrink: {} -> {}",
        report.source_bytes,
        report.output_bytes
    );
}

#[test]
fn plan_names_a_converted_image_after_the_target_format() {
    let dir = test_dir("plan_image_name");
    let source = dir.join("cover.png");
    fs::write(&source, png(16, 16)).expect("image should be created");

    let options = ConvertOptions {
        format: TargetFormat::Jpeg,
        ..ConvertOptions::default()
    };
    let plan =
        plan_conversion(std::slice::from_ref(&source), &options).expect("plan should succeed");

    assert_eq!(plan.entries[0].kind, ConvertKind::Image);
    assert_eq!(plan.entries[0].output_name, "cover-compressed.jpg");
}

#[test]
fn plan_keeps_the_image_extension_when_the_format_is_kept() {
    let dir = test_dir("plan_image_keep");
    let source = dir.join("cover.png");
    fs::write(&source, png(16, 16)).expect("image should be created");

    let plan = plan_conversion(std::slice::from_ref(&source), &ConvertOptions::default())
        .expect("plan should succeed");

    assert_eq!(plan.entries[0].output_name, "cover-compressed.png");
}

#[test]
fn convert_rewrites_a_single_image_and_keeps_the_source() {
    let dir = test_dir("convert_image");
    let source = dir.join("cover.png");
    fs::write(&source, noisy_png(200, 200)).expect("image should be created");
    let before = fs::read(&source).expect("source should be readable");

    let options = ConvertOptions {
        format: TargetFormat::Jpeg,
        quality: 40,
        ..ConvertOptions::default()
    };
    let plan =
        plan_conversion(std::slice::from_ref(&source), &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_converted, 1);
    assert_eq!(report.entries_copied, 0);
    assert_eq!(report.output_name, "cover-compressed.jpg");
    assert_eq!(
        image::guess_format(&fs::read(&report.output).expect("output should be readable"))
            .expect("format"),
        image::ImageFormat::Jpeg
    );
    assert!(report.output_bytes < report.source_bytes);
    assert_eq!(
        fs::read(&source).expect("source should be readable"),
        before
    );
}

#[test]
fn convert_reports_an_image_it_cannot_decode() {
    let dir = test_dir("convert_image_broken");
    let source = dir.join("cover.png");
    fs::write(&source, b"not really a png").expect("file should be created");

    let options = ConvertOptions::default();
    let plan =
        plan_conversion(std::slice::from_ref(&source), &options).expect("plan should succeed");
    let error =
        convert_entry(&plan.entries[0], &options).expect_err("a broken image should be reported");

    assert_eq!(error.to_string(), "cover.png could not be decoded");
}

#[test]
fn plan_accepts_archives_and_images_together() {
    let dir = test_dir("plan_mixed");
    let archive = dir.join("volume-1.cbz");
    write_archive(&archive, &[("page-1.png", png(8, 8))]);
    let image = dir.join("cover.png");
    fs::write(&image, png(16, 16)).expect("image should be created");

    let plan = plan_conversion(&[archive, image], &ConvertOptions::default())
        .expect("plan should succeed");

    assert_eq!(plan.entries.len(), 2);
    assert_eq!(plan.entries[0].kind, ConvertKind::Archive);
    assert_eq!(plan.entries[1].kind, ConvertKind::Image);
}

#[test]
fn plan_rejects_two_sources_that_resolve_to_one_output() {
    let dir = test_dir("plan_collision");
    let jpg = dir.join("cover.jpg");
    let png_file = dir.join("cover.png");
    fs::write(&jpg, png(8, 8)).expect("image should be created");
    fs::write(&png_file, png(8, 8)).expect("image should be created");

    let options = ConvertOptions {
        format: TargetFormat::Png,
        ..ConvertOptions::default()
    };
    let error =
        plan_conversion(&[jpg, png_file], &options).expect_err("a collision should be rejected");

    assert_eq!(
        error.to_string(),
        "multiple files resolve to cover-compressed.png"
    );
}

#[test]
fn convert_keeps_the_original_when_re_encoding_would_grow_it() {
    let dir = test_dir("convert_keep_smaller");
    let archive = dir.join("volume-1.zip");
    // A harshly compressed source, so re-encoding at high quality must grow it.
    write_archive(&archive, &[("page-1.jpg", jpeg(200, 200, 5))]);
    let before = read_entry(&archive, "page-1.jpg");

    let options = ConvertOptions {
        format: TargetFormat::Keep,
        quality: 95,
        ..ConvertOptions::default()
    };
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_kept, 1);
    assert_eq!(report.images_converted, 0);
    assert_eq!(read_entry(&report.output, "page-1.jpg"), before);
}

#[test]
fn convert_still_rewrites_when_a_format_was_asked_for() {
    let dir = test_dir("convert_forced_format");
    let archive = dir.join("volume-1.zip");
    write_archive(&archive, &[("page-1.jpg", jpeg(200, 200, 5))]);

    let options = ConvertOptions {
        format: TargetFormat::Png,
        ..ConvertOptions::default()
    };
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    let report = convert_entry(&plan.entries[0], &options).expect("conversion should succeed");

    assert_eq!(report.images_converted, 1);
    assert_eq!(report.images_kept, 0);
    assert_eq!(archive_names(&report.output), ["page-1.png"]);
}

#[test]
fn convert_reports_progress_for_every_image() {
    use std::sync::Mutex;

    let dir = test_dir("convert_progress");
    let archive = dir.join("volume-1.zip");
    write_archive(
        &archive,
        &[
            ("page-1.png", png(16, 16)),
            ("page-2.png", png(16, 16)),
            ("page-3.png", png(16, 16)),
            ("info.txt", b"metadata".to_vec()),
        ],
    );

    let seen = Mutex::new(Vec::new());
    let options = ConvertOptions::default();
    let plan = plan_conversion(&[archive], &options).expect("plan should succeed");
    convert_entry_with_progress(&plan.entries[0], &options, &|done, total| {
        seen.lock().expect("progress lock").push((done, total));
    })
    .expect("conversion should succeed");

    let seen = seen.into_inner().expect("progress lock");
    assert!(seen.iter().all(|(_, total)| *total == 4), "{seen:?}");
    assert_eq!(seen.first(), Some(&(0, 4)));
    assert_eq!(seen.last(), Some(&(4, 4)));
    let mut reached = seen.iter().map(|(done, _)| *done).collect::<Vec<_>>();
    reached.sort_unstable();
    reached.dedup();
    assert_eq!(reached, [0, 1, 2, 3, 4]);
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

/// A JPEG at a chosen quality, for testing the keep-the-smaller rule.
fn jpeg(width: u32, height: u32, quality: u8) -> Vec<u8> {
    let mut seed = 99u32;
    let image = image::RgbImage::from_fn(width, height, |_, _| {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        image::Rgb([(seed >> 16) as u8, (seed >> 8) as u8, seed as u8])
    });

    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(&image)
        .expect("jpeg should encode");
    bytes
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
