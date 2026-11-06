#![forbid(unsafe_code)]

mod config;
mod git;
mod ingest;
mod ui;

use anyhow::{Result, bail};
use std::io::{self, IsTerminal};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") => println!("Aero Archive {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h") => println!(
            "Aero Archive — simulation data mission control\n\nUsage: aero-archive [--no-splash] [FILES...]\n       aero-archive --doctor\n       aero-archive --version\n\nLaunch without arguments for the interactive interface.\nNo credentials are stored by this application."
        ),
        Some("--doctor") => {
            let paths = config::Paths::new()?;
            let config = config::Config::load(&paths)?;
            println!("{}", git::doctor(&config).join("\n"));
            println!(
                "\nConfiguration: {}\nManaged data: {}",
                paths.config.display(),
                paths.data.display()
            );
            println!("\n{}", git::setup_help());
        }
        _ => {
            if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
                bail!("Open Aero Archive in a terminal, or use --doctor / --help.");
            }
            let skip = args.iter().any(|s| s == "--no-splash");
            let files = args
                .into_iter()
                .filter(|s| s != "--no-splash")
                .map(Into::into)
                .collect();
            ui::run(skip, files)?;
        }
    }
    Ok(())
}
