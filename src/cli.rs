use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::renamer::{rename_by_regex, rename_by_sequence};

#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub verbose: bool,

    #[arg(short, long, value_name = "PATH", global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Init,
    Run,
    #[command(subcommand)]
    Rename(RenameCommand),
}

#[derive(Debug, Subcommand)]
pub enum RenameCommand {
    Sequence {
        directory: PathBuf,
    },
    Regex {
        directory: PathBuf,
        pattern: String,
        replacement: String,
    },
}

impl Cli {
    pub fn parse_from_args<I, T>(args: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::try_parse_from(args)
    }
}

pub fn run(cli: Cli) -> std::io::Result<()> {
    match cli.command {
        Command::Init => {
            println!("Initializing project");
        }
        Command::Run => {
            println!("Running project");
        }
        Command::Rename(command) => match command {
            RenameCommand::Sequence { directory } => {
                rename_by_sequence(&directory)?;
            }
            RenameCommand::Regex {
                directory,
                pattern,
                replacement,
            } => {
                rename_by_regex(&directory, &pattern, &replacement)?;
            }
        },
    }

    Ok(())
}
