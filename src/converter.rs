//! Cloning an archive with its images re-encoded at a chosen format and quality.

use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use image::{DynamicImage, ImageEncoder, ImageFormat};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::browser::{is_archive, is_image_file};

/// Entries larger than this are copied through untouched rather than decoded,
/// so one absurd image cannot exhaust memory.
pub const MAX_SOURCE_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TargetFormat {
    /// Re-encode each image in the format it already uses.
    #[default]
    Keep,
    Jpeg,
    Png,
    /// Lossless WebP; `quality` does not apply.
    Webp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct ConvertOptions {
    pub format: TargetFormat,
    /// JPEG quality, 1-100. Ignored by the lossless formats.
    pub quality: u8,
    /// Appended to the file stem to name the clone.
    pub suffix: String,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            format: TargetFormat::Keep,
            quality: 80,
            suffix: "-compressed".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConvertKind {
    /// A zip/cbz cloned with every image inside it re-encoded.
    Archive,
    /// A single image file re-encoded into a new file.
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConvertEntry {
    pub kind: ConvertKind,
    pub source: PathBuf,
    pub source_name: String,
    pub output: PathBuf,
    pub output_name: String,
    #[serde(default = "selected_by_default")]
    pub selected: bool,
    /// Why this entry cannot run, when it cannot. A blocked entry is never
    /// selected, so one collision does not sink the rest of the batch.
    #[serde(default)]
    pub blocked: Option<String>,
}

fn selected_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConvertPlan {
    pub entries: Vec<ConvertEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConvertReport {
    pub output: PathBuf,
    pub output_name: String,
    /// Images decoded and written back in the target format.
    pub images_converted: usize,
    /// Images that could not be decoded or encoded, copied through unchanged.
    pub images_skipped: usize,
    /// Images whose re-encode came out larger, so the original was kept.
    pub images_kept: usize,
    /// Non-image entries copied verbatim.
    pub entries_copied: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
}

/// Plans the converted copies for a selection without touching the disk.
///
/// Archives are cloned with their images re-encoded; image files are
/// re-encoded on their own.
pub fn plan_conversion(
    files: &[PathBuf],
    options: &ConvertOptions,
) -> Result<ConvertPlan, ConvertError> {
    validate_options(options)?;

    if files.is_empty() {
        return Err(ConvertError::Rejected {
            message: "nothing selected".to_string(),
        });
    }

    let mut entries = Vec::with_capacity(files.len());

    for source in files {
        let kind = if is_archive(source) {
            ConvertKind::Archive
        } else if is_image_file(source) {
            ConvertKind::Image
        } else {
            return Err(ConvertError::Rejected {
                message: format!("{} is not an archive or an image", display_name(source)),
            });
        };

        // An archive keeps its own extension; a converted image takes the
        // extension of the format it is written in.
        let extension = match kind {
            ConvertKind::Archive => None,
            ConvertKind::Image => image_output_extension(source, options),
        };
        let output = clone_path(source, &options.suffix, extension);
        let blocked = output.exists().then(|| "already exists".to_string());

        entries.push(ConvertEntry {
            kind,
            source_name: display_name(source),
            output_name: display_name(&output),
            source: source.clone(),
            selected: blocked.is_none(),
            output,
            blocked,
        });
    }

    let outputs = entries
        .iter()
        .map(|entry| entry.output.clone())
        .collect::<Vec<_>>();

    for index in 0..entries.len() {
        let collides = outputs
            .iter()
            .enumerate()
            .any(|(other, output)| other != index && *output == outputs[index]);

        if collides {
            entries[index].blocked = Some("two files resolve to this name".to_string());
            entries[index].selected = false;
        }
    }

    Ok(ConvertPlan { entries })
}

/// Reports how far through an entry the conversion is.
pub type ProgressFn<'a> = &'a (dyn Fn(usize, usize) + Sync);

/// Writes `entry.output`, leaving `entry.source` untouched.
pub fn convert_entry(
    entry: &ConvertEntry,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    convert_entry_with_progress(entry, options, &|_, _| {})
}

/// Same as [`convert_entry`], reporting `(done, total)` as images complete.
///
/// The callback runs on worker threads, so it must be cheap and thread safe.
pub fn convert_entry_with_progress(
    entry: &ConvertEntry,
    options: &ConvertOptions,
    progress: ProgressFn<'_>,
) -> Result<ConvertReport, ConvertError> {
    validate_options(options)?;

    if entry.output.exists() {
        return Err(ConvertError::Rejected {
            message: format!("{} already exists", display_name(&entry.output)),
        });
    }

    match entry.kind {
        ConvertKind::Archive => convert_archive(entry, options, progress),
        ConvertKind::Image => {
            let report = convert_image_file(entry, options)?;
            progress(1, 1);
            Ok(report)
        }
    }
}

/// Re-encodes one image file into `entry.output`.
fn convert_image_file(
    entry: &ConvertEntry,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    let bytes = fs::read(&entry.source).map_err(ConvertError::Io)?;
    let source_bytes = bytes.len() as u64;

    let Some((data, _)) = convert_image(&bytes, options) else {
        return Err(ConvertError::Rejected {
            message: format!("{} could not be decoded", entry.source_name),
        });
    };

    fs::write(&entry.output, &data).map_err(ConvertError::Io)?;

    Ok(ConvertReport {
        output: entry.output.clone(),
        output_name: entry.output_name.clone(),
        images_converted: 1,
        images_skipped: 0,
        images_kept: 0,
        entries_copied: 0,
        source_bytes,
        output_bytes: data.len() as u64,
    })
}

/// Clones the archive with every image inside it re-encoded.
///
/// Entries are read in order, converted across all cores, then written back in
/// their original order. Only one batch is held in memory at a time, so a large
/// archive does not have to fit in RAM.
///
/// An image that cannot be decoded is copied through unchanged, so a single bad
/// page never costs the rest of the archive.
fn convert_archive(
    entry: &ConvertEntry,
    options: &ConvertOptions,
    progress: ProgressFn<'_>,
) -> Result<ConvertReport, ConvertError> {
    let source_bytes = fs::metadata(&entry.source).map_err(ConvertError::Io)?.len();
    let file = fs::File::open(&entry.source).map_err(ConvertError::Io)?;
    let mut zip = zip::ZipArchive::new(file).map_err(archive_error)?;

    let indices = (0..zip.len())
        .filter(|index| {
            zip.name_for_index(*index)
                .is_some_and(crate::browser::is_content_entry)
        })
        .collect::<Vec<_>>();
    let total = indices.len();
    let done = AtomicUsize::new(0);
    progress(0, total);

    let mut report = ConvertReport {
        output: entry.output.clone(),
        output_name: entry.output_name.clone(),
        images_converted: 0,
        images_skipped: 0,
        images_kept: 0,
        entries_copied: 0,
        source_bytes,
        output_bytes: 0,
    };

    let output = fs::File::create(&entry.output).map_err(ConvertError::Io)?;
    let mut writer = zip::ZipWriter::new(BufWriter::new(output));

    // Two batches per thread keeps every core fed without holding the whole
    // archive in memory.
    let batch = rayon::current_num_threads().max(1) * 2;

    for chunk in indices.chunks(batch) {
        let mut raw = Vec::with_capacity(chunk.len());

        for &index in chunk {
            let mut source_entry = zip.by_index(index).map_err(archive_error)?;
            let name = source_entry.name().to_string();
            let size = source_entry.size();
            let mut bytes = Vec::with_capacity(size as usize);
            source_entry
                .read_to_end(&mut bytes)
                .map_err(ConvertError::Io)?;
            raw.push((name, bytes, size));
        }

        let converted = raw
            .into_par_iter()
            .map(|(name, bytes, size)| {
                let outcome = if size <= MAX_SOURCE_IMAGE_BYTES {
                    reencode(&name, &bytes, options)
                } else {
                    Outcome::Copied
                };

                progress(done.fetch_add(1, Ordering::Relaxed) + 1, total);
                (name, bytes, outcome)
            })
            .collect::<Vec<_>>();

        for (name, bytes, outcome) in converted {
            match outcome {
                Outcome::Converted(data, extension) => {
                    report.images_converted += 1;
                    write_entry(
                        &mut writer,
                        &replace_extension(&name, extension),
                        &data,
                        true,
                    )?;
                }
                Outcome::Kept => {
                    report.images_kept += 1;
                    write_entry(&mut writer, &name, &bytes, true)?;
                }
                Outcome::Undecodable => {
                    report.images_skipped += 1;
                    write_entry(&mut writer, &name, &bytes, true)?;
                }
                Outcome::Copied => {
                    report.entries_copied += 1;
                    write_entry(&mut writer, &name, &bytes, false)?;
                }
            }
        }
    }

    let mut buffered = writer.finish().map_err(archive_error)?;
    buffered.flush().map_err(ConvertError::Io)?;
    drop(buffered);

    report.output_bytes = fs::metadata(&entry.output).map_err(ConvertError::Io)?.len();
    progress(total, total);

    Ok(report)
}

enum Outcome {
    Converted(Vec<u8>, &'static str),
    /// Re-encoding made the image larger, so the original bytes win.
    Kept,
    /// Not a decodable image; the original bytes are copied.
    Undecodable,
    /// Not an image at all.
    Copied,
}

fn reencode(name: &str, bytes: &[u8], options: &ConvertOptions) -> Outcome {
    if !crate::browser::is_image_entry(name) {
        return Outcome::Copied;
    }

    let Some((data, extension)) = convert_image(bytes, options) else {
        return Outcome::Undecodable;
    };

    // Keeping the original format is a request to shrink, not to rewrite, so a
    // re-encode that grew is not worth taking. An explicit format change is.
    if options.format == TargetFormat::Keep && data.len() >= bytes.len() {
        return Outcome::Kept;
    }

    Outcome::Converted(data, extension)
}

#[derive(Debug)]
pub enum ConvertError {
    Io(io::Error),
    Archive { message: String },
    Rejected { message: String },
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Archive { message } | Self::Rejected { message } => {
                write!(formatter, "{message}")
            }
        }
    }
}

impl std::error::Error for ConvertError {}

fn archive_error(error: zip::result::ZipError) -> ConvertError {
    ConvertError::Archive {
        message: error.to_string(),
    }
}

fn validate_options(options: &ConvertOptions) -> Result<(), ConvertError> {
    if !(1..=100).contains(&options.quality) {
        return Err(ConvertError::Rejected {
            message: "quality must be between 1 and 100".to_string(),
        });
    }

    if options.suffix.is_empty() {
        return Err(ConvertError::Rejected {
            message: "suffix must not be empty".to_string(),
        });
    }

    if options.suffix.contains('/') || options.suffix.contains('\\') {
        return Err(ConvertError::Rejected {
            message: "suffix must not contain path separators".to_string(),
        });
    }

    Ok(())
}

/// `volume-1.cbz` with suffix `-compressed` becomes `volume-1-compressed.cbz`.
/// `extension` overrides the source extension when the format changes.
fn clone_path(source: &Path, suffix: &str, extension: Option<&str>) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = extension.or_else(|| source.extension().and_then(|value| value.to_str()));
    let name = match extension {
        Some(extension) => format!("{stem}{suffix}.{extension}"),
        None => format!("{stem}{suffix}"),
    };

    source.with_file_name(name)
}

/// The extension a converted image file will carry.
fn image_output_extension(source: &Path, options: &ConvertOptions) -> Option<&'static str> {
    match options.format {
        TargetFormat::Keep => source
            .extension()
            .and_then(|extension| extension.to_str())
            .and_then(ImageFormat::from_extension)
            .map(extension_for),
        TargetFormat::Jpeg => Some("jpg"),
        TargetFormat::Png => Some("png"),
        TargetFormat::Webp => Some("webp"),
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// Returns the re-encoded bytes and the extension they should carry, or `None`
/// when the entry is not a decodable image.
fn convert_image(bytes: &[u8], options: &ConvertOptions) -> Option<(Vec<u8>, &'static str)> {
    let source_format = image::guess_format(bytes).ok()?;
    let target = match options.format {
        TargetFormat::Keep => source_format,
        TargetFormat::Jpeg => ImageFormat::Jpeg,
        TargetFormat::Png => ImageFormat::Png,
        TargetFormat::Webp => ImageFormat::WebP,
    };
    let image = image::load_from_memory_with_format(bytes, source_format).ok()?;
    let encoded = encode(&image, target, options.quality)?;

    Some((encoded, extension_for(target)))
}

fn encode(image: &DynamicImage, format: ImageFormat, quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::new();

    match format {
        ImageFormat::Jpeg => {
            // JPEG carries no alpha channel.
            let rgb = image.to_rgb8();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality)
                .write_image(
                    rgb.as_raw(),
                    rgb.width(),
                    rgb.height(),
                    image::ExtendedColorType::Rgb8,
                )
                .ok()?;
        }
        ImageFormat::Png => {
            let (bytes, color) = opaque_or_alpha(image);
            image::codecs::png::PngEncoder::new(&mut out)
                .write_image(&bytes, image.width(), image.height(), color)
                .ok()?;
        }
        ImageFormat::WebP => {
            let (bytes, color) = opaque_or_alpha(image);
            image::codecs::webp::WebPEncoder::new_lossless(&mut out)
                .write_image(&bytes, image.width(), image.height(), color)
                .ok()?;
        }
        // Anything else keeps its original bytes.
        _ => return None,
    }

    Some(out)
}

/// Keeps an image at three channels unless it actually carries alpha.
/// Promoting RGB to RGBA inflates the encoded file by a third for nothing.
fn opaque_or_alpha(image: &DynamicImage) -> (Vec<u8>, image::ExtendedColorType) {
    if image.color().has_alpha() {
        (image.to_rgba8().into_raw(), image::ExtendedColorType::Rgba8)
    } else {
        (image.to_rgb8().into_raw(), image::ExtendedColorType::Rgb8)
    }
}

fn extension_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        ImageFormat::Gif => "gif",
        _ => "bmp",
    }
}

fn replace_extension(name: &str, extension: &str) -> String {
    match name.rfind('.') {
        Some(dot) if dot > name.rfind('/').map_or(0, |slash| slash) => {
            format!("{}.{extension}", &name[..dot])
        }
        _ => format!("{name}.{extension}"),
    }
}

fn write_entry(
    writer: &mut zip::ZipWriter<BufWriter<fs::File>>,
    name: &str,
    bytes: &[u8],
    store: bool,
) -> Result<(), ConvertError> {
    // Already-compressed image bytes gain nothing from deflate.
    let method = if store {
        zip::CompressionMethod::Stored
    } else {
        zip::CompressionMethod::Deflated
    };
    let options = zip::write::SimpleFileOptions::default().compression_method(method);

    writer.start_file(name, options).map_err(archive_error)?;
    writer.write_all(bytes).map_err(ConvertError::Io)?;

    Ok(())
}
