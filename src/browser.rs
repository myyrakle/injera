//! Directory browsing and archive previews for the in-app file list.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};

use crate::natural::natural_cmp;

/// Largest embedded preview image. Bigger entries are reported as skipped
/// rather than pushed through the webview bridge.
pub const MAX_PREVIEW_BYTES: u64 = 12 * 1024 * 1024;

const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "gif", "webp", "bmp", "avif"];
const ARCHIVE_EXTENSIONS: [&str; 2] = ["zip", "cbz"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    /// Whether the file can be opened for an archive preview.
    pub is_archive: bool,
    /// Whether the file is an image the converter can re-encode.
    pub is_image: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DirectoryListing {
    pub path: PathBuf,
    pub parent: Option<PathBuf>,
    pub directories: Vec<DirectoryEntry>,
    pub files: Vec<FileEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArchivePreview {
    /// Name of the previewed entry inside the archive.
    pub entry_name: String,
    /// `data:` URI holding the image bytes, ready for an `img` element.
    pub data_uri: String,
}

/// Directory the browser opens on first launch.
pub fn default_directory() -> PathBuf {
    // An Android app's HOME is its own sandbox, which holds none of the user's
    // files. Shared storage is where the archives actually are.
    #[cfg(target_os = "android")]
    for candidate in ["/storage/emulated/0", "/sdcard"] {
        let path = PathBuf::from(candidate);
        if path.is_dir() {
            return path;
        }
    }

    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Turns a bare `os error 13` into something that says what to do about it.
fn explain(directory: &Path, error: io::Error) -> io::Error {
    if error.kind() == io::ErrorKind::PermissionDenied {
        return io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{} cannot be read. Grant this app access to the folder and try again.",
                directory.display()
            ),
        );
    }

    error
}

/// Lists the sub-directories and files of `directory` in natural order.
///
/// Entries the process cannot stat are skipped rather than failing the listing,
/// so an unreadable file does not hide the rest of the folder.
pub fn list_directory(directory: &Path) -> io::Result<DirectoryListing> {
    let path = directory
        .canonicalize()
        .map_err(|error| explain(directory, error))?;

    if !path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("{} is not a directory", path.display()),
        ));
    }

    let mut directories = Vec::new();
    let mut files = Vec::new();

    for entry in fs::read_dir(&path).map_err(|error| explain(&path, error))? {
        let Ok(entry) = entry else { continue };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let entry_path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        if file_type.is_dir() {
            directories.push(DirectoryEntry {
                path: entry_path,
                name,
            });
        } else if file_type.is_file() {
            let size = entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
            files.push(FileEntry {
                is_archive: is_archive(&entry_path),
                is_image: is_image_file(&entry_path),
                path: entry_path,
                name,
                size,
            });
        }
    }

    directories.sort_by(|left, right| natural_cmp(&left.name, &right.name));
    files.sort_by(|left, right| natural_cmp(&left.name, &right.name));

    Ok(DirectoryListing {
        parent: path.parent().map(Path::to_path_buf),
        path,
        directories,
        files,
    })
}

/// Reads the first image inside `archive` in natural order.
///
/// Returns `Ok(None)` when the archive holds no image small enough to preview.
pub fn first_archive_image(archive: &Path) -> Result<Option<ArchivePreview>, ArchiveError> {
    let file = fs::File::open(archive).map_err(ArchiveError::Io)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|error| ArchiveError::Archive {
        message: error.to_string(),
    })?;

    let mut names = zip
        .file_names()
        .filter(|name| is_image_entry(name))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    names.sort_by(|left, right| natural_cmp(left, right));

    for name in names {
        let mut entry = zip.by_name(&name).map_err(|error| ArchiveError::Archive {
            message: error.to_string(),
        })?;

        if entry.size() > MAX_PREVIEW_BYTES {
            continue;
        }

        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(ArchiveError::Io)?;

        return Ok(Some(ArchivePreview {
            data_uri: format!(
                "data:{};base64,{}",
                mime_for(&name),
                STANDARD.encode(&bytes)
            ),
            entry_name: name,
        }));
    }

    Ok(None)
}

#[derive(Debug)]
pub enum ArchiveError {
    Io(io::Error),
    Archive { message: String },
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Archive { message } => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

/// Whether a path names an archive this app can open.
pub fn is_archive(path: &Path) -> bool {
    has_extension(path, &ARCHIVE_EXTENSIONS)
}

/// Whether a path names an image the converter can re-encode.
pub fn is_image_file(path: &Path) -> bool {
    has_extension(path, &IMAGE_EXTENSIONS)
}

/// Whether an archive entry is real content rather than a directory marker or
/// the metadata macOS adds when zipping.
pub(crate) fn is_content_entry(name: &str) -> bool {
    !name.ends_with('/')
        && !name.starts_with("__MACOSX/")
        && !name
            .rsplit('/')
            .next()
            .is_some_and(|leaf| leaf.starts_with("._") || leaf.eq_ignore_ascii_case(".ds_store"))
}

pub(crate) fn is_image_entry(name: &str) -> bool {
    is_content_entry(name) && has_extension(Path::new(name), &IMAGE_EXTENSIONS)
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extensions
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

fn mime_for(name: &str) -> &'static str {
    let extension = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        _ => "image/jpeg",
    }
}
