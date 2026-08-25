use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::natural::natural_cmp;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameEntry {
    pub source: PathBuf,
    pub target: PathBuf,
    pub source_name: String,
    pub target_name: String,
    /// Whether `apply_rename_plan` should rename this entry. Entries left
    /// unselected keep their current name and block that name as a target.
    #[serde(default = "selected_by_default")]
    pub selected: bool,
}

fn selected_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenamePlan {
    pub directory: PathBuf,
    pub entries: Vec<RenameEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenameReport {
    pub renamed_count: usize,
}

/// Smallest zero padding used when [`SequenceOptions::padding`] is `None`.
pub const DEFAULT_SEQUENCE_PADDING: usize = 5;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct SequenceOptions {
    /// Text placed before the number, such as `photo-`.
    pub prefix: String,
    /// Number given to the first file.
    pub start: usize,
    /// Fixed zero padding width, or `None` to size it from the largest number.
    pub padding: Option<usize>,
}

impl Default for SequenceOptions {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            start: 1,
            padding: None,
        }
    }
}

pub fn rename_by_sequence(directory: &Path) -> io::Result<()> {
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    rename_by_sequence_with_writer(directory, &mut writer)
}

pub fn rename_by_sequence_with_writer(directory: &Path, writer: &mut impl Write) -> io::Result<()> {
    writeln!(writer, "Scanning {}", directory.display())?;
    let plan = plan_sequence_rename(directory)?;
    writeln!(writer, "Found {} files", plan.entries.len())?;

    log_plan(writer, &plan)?;
    apply_rename_plan(&plan)?;
    writeln!(writer, "Done")?;

    Ok(())
}

pub fn plan_sequence_rename(directory: &Path) -> io::Result<RenamePlan> {
    plan_sequence_rename_with_options(directory, &SequenceOptions::default())
}

pub fn plan_sequence_rename_with_options(
    directory: &Path,
    options: &SequenceOptions,
) -> io::Result<RenamePlan> {
    let files = sorted_files(directory)?;
    plan_sequence_in(directory, files, options)
}

/// Plans a sequence rename over an explicit selection of files.
///
/// The files must all sit in one directory; they are numbered in natural order
/// regardless of the order they are passed in.
pub fn plan_sequence_rename_for_files(
    files: &[PathBuf],
    options: &SequenceOptions,
) -> io::Result<RenamePlan> {
    let (directory, files) = normalize_selection(files)?;
    plan_sequence_in(&directory, files, options)
}

fn plan_sequence_in(
    directory: &Path,
    files: Vec<PathBuf>,
    options: &SequenceOptions,
) -> io::Result<RenamePlan> {
    validate_prefix(&options.prefix)?;

    let width = sequence_width(options, files.len());
    let targets = files
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let number = options.start.saturating_add(index);
            let number = format!("{number:0width$}");
            let prefix = &options.prefix;
            let file_name = match path.extension().and_then(|extension| extension.to_str()) {
                Some(extension) => format!("{prefix}{number}.{extension}"),
                None => format!("{prefix}{number}"),
            };

            directory.join(file_name)
        })
        .collect::<Vec<_>>();

    validate_unique_targets(&targets).map_err(io::Error::other)?;
    validate_available_targets(&files, &targets)?;

    Ok(rename_plan(directory, files, targets))
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
    let plan = plan_regex_rename(directory, pattern, replacement)?;
    writeln!(writer, "Found {} files", plan.entries.len()).map_err(RenameError::Io)?;

    log_plan(writer, &plan).map_err(RenameError::Io)?;
    apply_rename_plan(&plan).map_err(RenameError::Io)?;
    writeln!(writer, "Done").map_err(RenameError::Io)?;

    Ok(())
}

pub fn plan_regex_rename(
    directory: &Path,
    pattern: &str,
    replacement: &str,
) -> Result<RenamePlan, RenameError> {
    let files = sorted_files(directory).map_err(RenameError::Io)?;
    plan_regex_in(directory, files, pattern, replacement)
}

/// Plans a regex rename over an explicit selection of files.
pub fn plan_regex_rename_for_files(
    files: &[PathBuf],
    pattern: &str,
    replacement: &str,
) -> Result<RenamePlan, RenameError> {
    let (directory, files) = normalize_selection(files).map_err(RenameError::Io)?;
    plan_regex_in(&directory, files, pattern, replacement)
}

fn plan_regex_in(
    directory: &Path,
    files: Vec<PathBuf>,
    pattern: &str,
    replacement: &str,
) -> Result<RenamePlan, RenameError> {
    let regex = Regex::new(pattern).map_err(RenameError::InvalidRegex)?;
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

    Ok(rename_plan(directory, files, targets))
}

pub fn apply_rename_plan(plan: &RenamePlan) -> io::Result<RenameReport> {
    let sources = plan
        .entries
        .iter()
        .filter(|entry| entry.selected)
        .map(|entry| entry.source.clone())
        .collect::<Vec<_>>();
    let targets = plan
        .entries
        .iter()
        .filter(|entry| entry.selected)
        .map(|entry| entry.target.clone())
        .collect::<Vec<_>>();

    validate_unique_targets(&targets).map_err(io::Error::other)?;
    validate_available_targets(&sources, &targets)?;
    rename_all(&sources, &targets)?;

    Ok(RenameReport {
        renamed_count: sources
            .iter()
            .zip(targets.iter())
            .filter(|(source, target)| source != target)
            .count(),
    })
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
            .unwrap_or(self.target.as_os_str())
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
            .unwrap_or(self.target.as_os_str())
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

/// Sorts a selection naturally and returns the single directory holding it.
fn normalize_selection(files: &[PathBuf]) -> io::Result<(PathBuf, Vec<PathBuf>)> {
    let Some(first) = files.first() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "no files selected",
        ));
    };

    let directory = first
        .parent()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "selected files must have a parent directory",
            )
        })?
        .to_path_buf();

    if files.iter().any(|file| file.parent() != Some(&directory)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "selected files must share one directory",
        ));
    }

    let mut files = files.to_vec();
    files.sort_by(|left, right| {
        natural_cmp(&display_file_name(left), &display_file_name(right))
            .then_with(|| display_file_name(left).cmp(&display_file_name(right)))
    });

    Ok((directory, files))
}

fn sequence_width(options: &SequenceOptions, count: usize) -> usize {
    match options.padding {
        Some(padding) => padding,
        None => options
            .start
            .saturating_add(count.saturating_sub(1))
            .to_string()
            .len()
            .max(DEFAULT_SEQUENCE_PADDING),
    }
}

fn validate_prefix(prefix: &str) -> io::Result<()> {
    if prefix.contains('/') || prefix.contains('\\') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "prefix must not contain path separators",
        ));
    }

    Ok(())
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

fn log_plan(writer: &mut impl Write, plan: &RenamePlan) -> io::Result<()> {
    for (index, entry) in plan.entries.iter().enumerate() {
        writeln!(
            writer,
            "[{}/{}] {} -> {}",
            index + 1,
            plan.entries.len(),
            entry.source_name,
            entry.target_name,
        )?;
    }

    Ok(())
}

fn rename_plan(directory: &Path, sources: Vec<PathBuf>, targets: Vec<PathBuf>) -> RenamePlan {
    let entries = sources
        .into_iter()
        .zip(targets)
        .map(|(source, target)| RenameEntry {
            source_name: display_file_name(&source),
            target_name: display_file_name(&target),
            source,
            target,
            selected: true,
        })
        .collect();

    RenamePlan {
        directory: directory.to_path_buf(),
        entries,
    }
}

fn display_file_name(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
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
