use std::{
    ffi::{OsStr, OsString},
    fs::{create_dir_all, canonicalize},
    path::PathBuf,
};

use shuttle_common::project::ProjectName;
use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(structopt::clap::Arg::with_name("dummy")
        .possible_value("shuttle")
        .required(false)
        .hidden(true))
)]
pub struct Args {
    #[structopt(
        long,
        about = "allows targeting a custom deloyed instance for this command only",
        env = "SHUTTLE_API"
    )]
    /// Run this command against the api at the supplied url
    pub api_url: Option<String>,
    #[structopt(flatten)]
    pub project_args: ProjectArgs,
    #[structopt(subcommand)]
    pub cmd: Command,
}

// Common args for subcommands that deal with projects.
#[derive(StructOpt)]
pub struct ProjectArgs {
    #[structopt(
        global = true,
        long,
        parse(try_from_os_str = parse_path),
        default_value = ".",
    )]
    /// Specify the working directory
    pub working_directory: PathBuf,
    #[structopt(global = true, long)]
    /// Specify the name of the project (overrides crate name)
    pub name: Option<ProjectName>,
}

#[derive(StructOpt)]
pub enum Command {
    #[structopt(about = "create user credentials for the shuttle platform")]
    Auth(AuthArgs),
    #[structopt(about = "deploy a shuttle project")]
    Deploy(DeployArgs),
    #[structopt(about = "delete the latest deployment for a shuttle project")]
    Delete,
    #[structopt(about = "create a new shuttle project in an existing directory")]
    Init(InitArgs),
    #[structopt(about = "login to the shuttle platform")]
    Login(LoginArgs),
    #[structopt(about = "view the logs of a shuttle project")]
    Logs,
    #[structopt(about = "view the status of a shuttle project")]
    Status,
}

#[derive(StructOpt)]
pub struct LoginArgs {
    #[structopt(long, about = "api key for the shuttle platform")]
    pub api_key: Option<String>,
}

#[derive(StructOpt)]
pub struct AuthArgs {
    #[structopt(about = "the desired username for the shuttle platform")]
    pub username: String,
}

#[derive(StructOpt)]
pub struct DeployArgs {
    #[structopt(long, about = "allow dirty working directories to be packaged")]
    pub allow_dirty: bool,
}

#[derive(StructOpt)]
pub struct InitArgs {
    #[structopt(
        about = "the path to initialize",
        parse(try_from_os_str = parse_init_path),
        default_value = ".",
    )]
    pub path: PathBuf,
}

// Helper function to parse and return the absolute path
fn parse_path(path: &OsStr) -> Result<PathBuf, OsString> {
    canonicalize(path)
        .map_err(|e| format!("could not turn {path:?} into a real path: {e}").into())
}

// Helper function to parse, create if not exists, and return the absolute path
fn parse_init_path(path: &OsStr) -> Result<PathBuf, OsString> {
    // Create the directory if does not exist
    create_dir_all(path).expect("could not find or create a directory with the given path");

    parse_path(path)
}
