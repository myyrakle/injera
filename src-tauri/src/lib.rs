use std::path::PathBuf;

use injera::browser::{
    ArchivePreview, DirectoryListing, default_directory, first_archive_image,
    list_directory as list_directory_entries,
};
use injera::converter::{
    ConvertEntry, ConvertOptions, ConvertPlan, ConvertReport, convert_entry as convert_one,
    plan_conversion,
};
use injera::renamer::{
    RenamePlan, RenameReport, SequenceOptions, apply_rename_plan, plan_regex_rename_for_files,
    plan_sequence_rename_for_files,
};

/// Lists a directory for the in-app browser. `path` is `None` on first load.
#[tauri::command]
async fn list_directory(path: Option<String>) -> Result<DirectoryListing, String> {
    let directory = path.map(PathBuf::from).unwrap_or_else(default_directory);
    list_directory_entries(&directory).map_err(|error| error.to_string())
}

/// Reads the first image out of an archive so the browser can show a thumbnail.
#[tauri::command]
async fn archive_preview(path: String) -> Result<Option<ArchivePreview>, String> {
    tauri::async_runtime::spawn_blocking(move || first_archive_image(&PathBuf::from(path)))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_sequence(
    files: Vec<String>,
    options: Option<SequenceOptions>,
) -> Result<RenamePlan, String> {
    let files = to_paths(files);
    let options = options.unwrap_or_default();
    plan_sequence_rename_for_files(&files, &options).map_err(|error| error.to_string())
}

#[tauri::command]
async fn preview_regex(
    files: Vec<String>,
    pattern: String,
    replacement: String,
) -> Result<RenamePlan, String> {
    let files = to_paths(files);
    plan_regex_rename_for_files(&files, &pattern, &replacement).map_err(|error| error.to_string())
}

/// Plans the converted copies for the selected archives and images.
#[tauri::command]
async fn preview_conversion(
    files: Vec<String>,
    options: Option<ConvertOptions>,
) -> Result<ConvertPlan, String> {
    let files = to_paths(files);
    let options = options.unwrap_or_default();
    plan_conversion(&files, &options).map_err(|error| error.to_string())
}

/// Writes one converted copy. The frontend calls this per entry so progress
/// shows up while a long batch runs.
#[tauri::command]
async fn convert_entry(
    entry: ConvertEntry,
    options: Option<ConvertOptions>,
) -> Result<ConvertReport, String> {
    let options = options.unwrap_or_default();
    tauri::async_runtime::spawn_blocking(move || convert_one(&entry, &options))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn apply_rename(plan: RenamePlan) -> Result<RenameReport, String> {
    apply_rename_plan(&plan).map_err(|error| error.to_string())
}

fn to_paths(files: Vec<String>) -> Vec<PathBuf> {
    files.into_iter().map(PathBuf::from).collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            list_directory,
            archive_preview,
            preview_sequence,
            preview_regex,
            apply_rename,
            preview_conversion,
            convert_entry
        ])
        .run(tauri::generate_context!())
        .expect("error while running injera tauri application");
}
