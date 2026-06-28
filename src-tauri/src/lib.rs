use std::path::PathBuf;

use injera::renamer::{
    RenamePlan, RenameReport, apply_rename_plan, plan_regex_rename, plan_sequence_rename,
};

#[tauri::command]
fn preview_sequence(directory: String) -> Result<RenamePlan, String> {
    plan_sequence_rename(&PathBuf::from(directory)).map_err(|error| error.to_string())
}

#[tauri::command]
fn preview_regex(
    directory: String,
    pattern: String,
    replacement: String,
) -> Result<RenamePlan, String> {
    plan_regex_rename(&PathBuf::from(directory), &pattern, &replacement)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_rename(plan: RenamePlan) -> Result<RenameReport, String> {
    apply_rename_plan(&plan).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            preview_sequence,
            preview_regex,
            apply_rename
        ])
        .run(tauri::generate_context!())
        .expect("error while running injera tauri application");
}
