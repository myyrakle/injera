use clap::Parser;
use rust_template::cli::{Cli, run};

fn main() {
    let cli = Cli::parse();
    run(cli);
}
