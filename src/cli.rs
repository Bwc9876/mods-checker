use std::path::PathBuf;

use clap_derive::{Parser, Subcommand};

#[derive(Parser)]
#[command(name="mods-checker", author, version, about, long_about = None)]
pub struct CheckerCli {
    #[command(subcommand)]
    pub command: CheckerSubcommand,
    #[arg(
        global = true,
        short = 'd',
        long = "skip-exists",
        help = "Don't check if this mod is already in the database"
    )]
    pub skip_exists: bool,
    #[arg(
        global = true,
        short = 'n',
        long = "expected-unique-name",
        help = "The expected unique name of the mod"
    )]
    pub expected_unique_name: Option<String>,
}

#[derive(Subcommand)]
pub enum CheckerSubcommand {
    #[command(
        name = "repo",
        about = "Check a mod from a github repository (`owner/name`)"
    )]
    Repo { repo: String },
    #[command(name = "url", about = "Check a mod from a direct download url")]
    Url { url: String },
    #[command(name = "file", about = "Check a mod from a local zip file")]
    File { file: PathBuf },
}
