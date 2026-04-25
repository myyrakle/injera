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
