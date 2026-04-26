use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use regex::Regex;

pub fn rename_by_sequence(directory: &Path) -> io::Result<()> {
    let files = sorted_files(directory)?;
    let width = files.len().to_string().len().max(5);

    let targets = files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let number = format!("{:0width$}", index + 1);
            let file_name = match path.extension().and_then(|extension| extension.to_str()) {
                Some(extension) => format!("{number}.{extension}"),
                None => number,
            };

            directory.join(file_name)
        })
        .collect::<Vec<_>>();

    validate_unique_targets(&targets).map_err(io::Error::other)?;
    validate_available_targets(&files, &targets)?;

    rename_all(&files, &targets)
}

pub fn rename_by_regex(
    directory: &Path,
    pattern: &str,
    replacement: &str,
) -> Result<(), RenameError> {
    let regex = Regex::new(pattern).map_err(RenameError::InvalidRegex)?;
    let files = sorted_files(directory).map_err(RenameError::Io)?;
    let targets = files
        .iter()
        .map(|path| {
            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default();
            directory.join(regex.replace_all(&file_name, replacement).as_ref())
        })
        .collect::<Vec<_>>();

    validate_unique_targets(&targets).map_err(RenameError::DuplicateTarget)?;
    validate_available_targets(&files, &targets).map_err(RenameError::Io)?;

    rename_all(&files, &targets).map_err(RenameError::Io)
}

#[derive(Debug)]
pub enum RenameError {
    Io(io::Error),
    InvalidRegex(regex::Error),
    DuplicateTarget(DuplicateTargetError),
}

#[derive(Debug)]
pub struct DuplicateTargetError {
    target: PathBuf,
}

#[derive(Debug)]
pub struct BlockedTargetError {
    target: PathBuf,
}

impl std::fmt::Display for RenameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::InvalidRegex(error) => write!(formatter, "{error}"),
            Self::DuplicateTarget(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for RenameError {}

impl std::fmt::Display for DuplicateTargetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let target = self
            .target
            .file_name()
            .unwrap_or_else(|| self.target.as_os_str())
            .to_string_lossy();
        write!(formatter, "multiple files resolve to {target}")
    }
}

impl std::error::Error for DuplicateTargetError {}

impl std::fmt::Display for BlockedTargetError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let target = self
            .target
            .file_name()
            .unwrap_or_else(|| self.target.as_os_str())
            .to_string_lossy();
        write!(formatter, "target already exists: {target}")
    }
}

impl std::error::Error for BlockedTargetError {}

impl From<RenameError> for io::Error {
    fn from(error: RenameError) -> Self {
        match error {
            RenameError::Io(error) => error,
            RenameError::InvalidRegex(error) => io::Error::new(io::ErrorKind::InvalidInput, error),
            RenameError::DuplicateTarget(error) => {
                io::Error::new(io::ErrorKind::AlreadyExists, error)
            }
        }
    }
}

fn sorted_files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(directory)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let file_type = entry.file_type().ok()?;
            file_type.is_file().then(|| entry.path())
        })
        .collect::<Vec<_>>();

    files.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(files)
}

fn validate_unique_targets(targets: &[PathBuf]) -> Result<(), DuplicateTargetError> {
    for (index, target) in targets.iter().enumerate() {
        if targets[index + 1..].iter().any(|other| other == target) {
            return Err(DuplicateTargetError {
                target: target.clone(),
            });
        }
    }

    Ok(())
}

fn validate_available_targets(sources: &[PathBuf], targets: &[PathBuf]) -> io::Result<()> {
    for target in targets {
        if target.exists() && !sources.iter().any(|source| source == target) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                BlockedTargetError {
                    target: target.clone(),
                },
            ));
        }
    }

    Ok(())
}

fn rename_all(sources: &[PathBuf], targets: &[PathBuf]) -> io::Result<()> {
    let temp_paths = sources
        .iter()
        .enumerate()
        .map(|(index, source)| source.with_file_name(format!(".injera-rename-{index}.tmp")))
        .collect::<Vec<_>>();

    for (source, temp_path) in sources.iter().zip(&temp_paths) {
        fs::rename(source, temp_path)?;
    }

    for (temp_path, target) in temp_paths.iter().zip(targets) {
        fs::rename(temp_path, target)?;
    }

    Ok(())
}
