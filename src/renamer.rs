use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::{cmp::Ordering, iter::Peekable, str::Chars};

use regex::Regex;

pub fn rename_by_sequence(directory: &Path) -> io::Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    rename_by_sequence_with_writer(directory, &mut writer)
}

pub fn rename_by_sequence_with_writer(directory: &Path, writer: &mut impl Write) -> io::Result<()> {
    writeln!(writer, "Scanning {}", directory.display())?;
    let files = sorted_files(directory)?;
    writeln!(writer, "Found {} files", files.len())?;

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

    log_plan(writer, &files, &targets)?;
    rename_all(&files, &targets)?;
    writeln!(writer, "Done")?;

    Ok(())
}

pub fn rename_by_regex(
    directory: &Path,
    pattern: &str,
    replacement: &str,
) -> Result<(), RenameError> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    rename_by_regex_with_writer(directory, pattern, replacement, &mut writer)
}

pub fn rename_by_regex_with_writer(
    directory: &Path,
    pattern: &str,
    replacement: &str,
    writer: &mut impl Write,
) -> Result<(), RenameError> {
    writeln!(writer, "Scanning {}", directory.display()).map_err(RenameError::Io)?;
    let regex = Regex::new(pattern).map_err(RenameError::InvalidRegex)?;
    let files = sorted_files(directory).map_err(RenameError::Io)?;
    writeln!(writer, "Found {} files", files.len()).map_err(RenameError::Io)?;

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

    log_plan(writer, &files, &targets).map_err(RenameError::Io)?;
    rename_all(&files, &targets).map_err(RenameError::Io)?;
    writeln!(writer, "Done").map_err(RenameError::Io)?;

    Ok(())
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

    files.sort_by(|left, right| {
        natural_cmp(&display_file_name(left), &display_file_name(right))
            .then_with(|| display_file_name(left).cmp(&display_file_name(right)))
    });
    Ok(files)
}

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut left = left.chars().peekable();
    let mut right = right.chars().peekable();

    loop {
        match (left.peek(), right.peek()) {
            (Some(left_char), Some(right_char))
                if left_char.is_ascii_digit() && right_char.is_ascii_digit() =>
            {
                let ordering = compare_number_chunks(&mut left, &mut right);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (Some(_), Some(_)) => {
                let left_char = left.next().expect("peeked char should exist");
                let right_char = right.next().expect("peeked char should exist");
                let ordering = left_char.cmp(&right_char);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => return Ordering::Equal,
        }
    }
}

fn compare_number_chunks(
    left: &mut Peekable<Chars<'_>>,
    right: &mut Peekable<Chars<'_>>,
) -> Ordering {
    let left_number = take_ascii_digits(left);
    let right_number = take_ascii_digits(right);
    let left_trimmed = left_number.trim_start_matches('0');
    let right_trimmed = right_number.trim_start_matches('0');
    let left_normalized = if left_trimmed.is_empty() {
        "0"
    } else {
        left_trimmed
    };
    let right_normalized = if right_trimmed.is_empty() {
        "0"
    } else {
        right_trimmed
    };

    left_normalized
        .len()
        .cmp(&right_normalized.len())
        .then_with(|| left_normalized.cmp(right_normalized))
        .then_with(|| left_number.len().cmp(&right_number.len()))
}

fn take_ascii_digits(chars: &mut Peekable<Chars<'_>>) -> String {
    let mut digits = String::new();

    while chars.peek().is_some_and(|char| char.is_ascii_digit()) {
        digits.push(chars.next().expect("peeked char should exist"));
    }

    digits
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

fn log_plan(writer: &mut impl Write, sources: &[PathBuf], targets: &[PathBuf]) -> io::Result<()> {
    for (index, (source, target)) in sources.iter().zip(targets).enumerate() {
        writeln!(
            writer,
            "[{}/{}] {} -> {}",
            index + 1,
            sources.len(),
            display_file_name(source),
            display_file_name(target),
        )?;
    }

    Ok(())
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or_else(|| path.as_os_str())
        .to_string_lossy()
        .into_owned()
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
