//! Cloning an archive with its images re-encoded at a chosen format and quality.

use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageEncoder, ImageFormat};
use serde::{Deserialize, Serialize};

use crate::browser::is_archive;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConvertEntry {
    pub source: PathBuf,
    pub source_name: String,
    pub output: PathBuf,
    pub output_name: String,
    #[serde(default = "selected_by_default")]
    pub selected: bool,
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
    /// Non-image entries copied verbatim.
    pub entries_copied: usize,
    pub source_bytes: u64,
    pub output_bytes: u64,
}

/// Plans the clones for a selection of archives without touching the disk.
pub fn plan_archive_conversion(
    files: &[PathBuf],
    options: &ConvertOptions,
) -> Result<ConvertPlan, ConvertError> {
    validate_options(options)?;

    if files.is_empty() {
        return Err(ConvertError::Rejected {
            message: "no archives selected".to_string(),
        });
    }

    let mut entries = Vec::with_capacity(files.len());

    for source in files {
        if !is_archive(source) {
            return Err(ConvertError::Rejected {
                message: format!("{} is not a zip archive", display_name(source)),
            });
        }

        let output = clone_path(source, &options.suffix);

        if output.exists() {
            return Err(ConvertError::Rejected {
                message: format!("{} already exists", display_name(&output)),
            });
        }

        entries.push(ConvertEntry {
            source_name: display_name(source),
            output_name: display_name(&output),
            source: source.clone(),
            output,
            selected: true,
        });
    }

    let outputs = entries
        .iter()
        .map(|entry| entry.output.clone())
        .collect::<Vec<_>>();

    for (index, output) in outputs.iter().enumerate() {
        if outputs[index + 1..].iter().any(|other| other == output) {
            return Err(ConvertError::Rejected {
                message: format!("multiple archives resolve to {}", display_name(output)),
            });
        }
    }

    Ok(ConvertPlan { entries })
}

/// Writes `entry.output`: a copy of the archive whose images are re-encoded.
///
/// Entries that cannot be decoded are copied through unchanged so a single bad
/// image never costs the rest of the archive.
pub fn convert_archive(
    entry: &ConvertEntry,
    options: &ConvertOptions,
) -> Result<ConvertReport, ConvertError> {
    validate_options(options)?;

    if entry.output.exists() {
        return Err(ConvertError::Rejected {
            message: format!("{} already exists", display_name(&entry.output)),
        });
    }

    let source_bytes = fs::metadata(&entry.source).map_err(ConvertError::Io)?.len();
    let file = fs::File::open(&entry.source).map_err(ConvertError::Io)?;
    let mut zip = zip::ZipArchive::new(file).map_err(archive_error)?;

    let mut buffer = Cursor::new(Vec::new());
    let mut report = ConvertReport {
        output: entry.output.clone(),
        output_name: entry.output_name.clone(),
        images_converted: 0,
        images_skipped: 0,
        entries_copied: 0,
        source_bytes,
        output_bytes: 0,
    };

    {
        let mut writer = zip::ZipWriter::new(&mut buffer);

        for index in 0..zip.len() {
            let mut source_entry = zip.by_index(index).map_err(archive_error)?;
            let name = source_entry.name().to_string();

            if !crate::browser::is_content_entry(&name) {
                continue;
            }

            let mut bytes = Vec::with_capacity(source_entry.size() as usize);
            source_entry
                .read_to_end(&mut bytes)
                .map_err(ConvertError::Io)?;

            let converted = if source_entry.size() <= MAX_SOURCE_IMAGE_BYTES {
                convert_image(&bytes, options)
            } else {
                None
            };

            match converted {
                Some((data, extension)) => {
                    report.images_converted += 1;
                    write_entry(
                        &mut writer,
                        &replace_extension(&name, extension),
                        &data,
                        true,
                    )?;
                }
                None => {
                    if crate::browser::is_image_entry(&name) {
                        report.images_skipped += 1;
                    } else {
                        report.entries_copied += 1;
                    }

                    let store = crate::browser::is_image_entry(&name);
                    write_entry(&mut writer, &name, &bytes, store)?;
                }
            }
        }

        writer.finish().map_err(archive_error)?;
    }

    let data = buffer.into_inner();
    report.output_bytes = data.len() as u64;
    fs::write(&entry.output, &data).map_err(ConvertError::Io)?;

    Ok(report)
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
fn clone_path(source: &Path, suffix: &str) -> PathBuf {
    let stem = source
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = match source.extension().and_then(|extension| extension.to_str()) {
        Some(extension) => format!("{stem}{suffix}.{extension}"),
        None => format!("{stem}{suffix}"),
    };

    source.with_file_name(name)
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
            let rgba = image.to_rgba8();
            image::codecs::png::PngEncoder::new(&mut out)
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .ok()?;
        }
        ImageFormat::WebP => {
            let rgba = image.to_rgba8();
            image::codecs::webp::WebPEncoder::new_lossless(&mut out)
                .write_image(
                    rgba.as_raw(),
                    rgba.width(),
                    rgba.height(),
                    image::ExtendedColorType::Rgba8,
                )
                .ok()?;
        }
        // Anything else keeps its original bytes.
        _ => return None,
    }

    Some(out)
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
    writer: &mut zip::ZipWriter<&mut Cursor<Vec<u8>>>,
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
