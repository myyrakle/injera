use rust_template::cli::{Cli, Command};

#[test]
fn parses_verbose_config_and_run_command() {
    let cli = Cli::parse_from_args([
        "rust_template",
        "--verbose",
        "--config",
        "config.toml",
        "run",
    ])
    .expect("CLI args should parse");

    assert!(cli.verbose);
    assert_eq!(
        cli.config.as_deref(),
        Some(std::path::Path::new("config.toml"))
    );
    assert!(matches!(cli.command, Command::Run));
}

#[test]
fn parses_init_command() {
    let cli = Cli::parse_from_args(["rust_template", "init"]).expect("CLI args should parse");

    assert!(!cli.verbose);
    assert!(cli.config.is_none());
    assert!(matches!(cli.command, Command::Init));
}

#[test]
fn parses_sequence_rename_command() {
    let cli = Cli::parse_from_args(["rust_template", "rename", "sequence", "fixtures"])
        .expect("CLI args should parse");

    assert!(matches!(
        cli.command,
        Command::Rename(rust_template::cli::RenameCommand::Sequence { ref directory })
            if directory == std::path::Path::new("fixtures")
    ));
}

#[test]
fn parses_regex_rename_command() {
    let cli = Cli::parse_from_args([
        "rust_template",
        "rename",
        "regex",
        "fixtures",
        r"^(.+)\.txt$",
        "$1.md",
    ])
    .expect("CLI args should parse");

    assert!(matches!(
        cli.command,
        Command::Rename(rust_template::cli::RenameCommand::Regex {
            ref directory,
            ref pattern,
            ref replacement,
        }) if directory == std::path::Path::new("fixtures")
            && pattern == r"^(.+)\.txt$"
            && replacement == "$1.md"
    ));
}
