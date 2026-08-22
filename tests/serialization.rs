//! Guards the JSON shape exchanged between the Tauri commands and the frontend.

use std::path::PathBuf;

use injera::renamer::{RenameEntry, RenamePlan, RenameReport, SequenceOptions};

#[test]
fn sequence_options_deserialize_from_frontend_payload() {
    let options: SequenceOptions =
        serde_json::from_str(r#"{"prefix":"photo-","start":8,"padding":3}"#)
            .expect("frontend payload should deserialize");

    assert_eq!(
        options,
        SequenceOptions {
            prefix: "photo-".to_string(),
            start: 8,
            padding: Some(3),
        }
    );
}

#[test]
fn sequence_options_read_null_padding_as_automatic() {
    let options: SequenceOptions =
        serde_json::from_str(r#"{"prefix":"","start":1,"padding":null}"#)
            .expect("automatic padding should deserialize");

    assert_eq!(options.padding, None);
    assert_eq!(options, SequenceOptions::default());
}

#[test]
fn sequence_options_fill_missing_fields_with_defaults() {
    let options: SequenceOptions =
        serde_json::from_str("{}").expect("empty payload should deserialize");

    assert_eq!(options, SequenceOptions::default());
}

#[test]
fn rename_entry_defaults_to_selected_when_field_is_absent() {
    let entry: RenameEntry = serde_json::from_str(
        r#"{"source":"/tmp/a.txt","target":"/tmp/00001.txt","source_name":"a.txt","target_name":"00001.txt"}"#,
    )
    .expect("entry without a selected field should deserialize");

    assert!(entry.selected);
}

#[test]
fn rename_plan_round_trips_through_json() {
    let plan = RenamePlan {
        directory: PathBuf::from("/tmp/photos"),
        entries: vec![
            RenameEntry {
                source: PathBuf::from("/tmp/photos/a.txt"),
                target: PathBuf::from("/tmp/photos/00001.txt"),
                source_name: "a.txt".to_string(),
                target_name: "00001.txt".to_string(),
                selected: true,
            },
            RenameEntry {
                source: PathBuf::from("/tmp/photos/b.txt"),
                target: PathBuf::from("/tmp/photos/00002.txt"),
                source_name: "b.txt".to_string(),
                target_name: "00002.txt".to_string(),
                selected: false,
            },
        ],
    };

    let json = serde_json::to_string(&plan).expect("plan should serialize");
    let restored: RenamePlan = serde_json::from_str(&json).expect("plan should deserialize");

    assert_eq!(plan, restored);
    assert!(json.contains("\"selected\":false"));
}

#[test]
fn rename_report_exposes_snake_case_count() {
    let json =
        serde_json::to_string(&RenameReport { renamed_count: 3 }).expect("report should serialize");

    assert_eq!(json, r#"{"renamed_count":3}"#);
}
