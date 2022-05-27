use std::{
    ffi::{OsStr, OsString},
    fs::canonicalize,
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
        parse(try_from_os_str = parse_working_directory),
        default_value = ".",
    )]
    /// Specify the working directory
    pub working_directory: PathBuf,
    #[structopt(global = true, long)]
    /// Specify the name of the project (overrides crate name)
    pub name: Option<ProjectName>,
}

fn parse_working_directory(working_directory: &OsStr) -> Result<PathBuf, OsString> {
    canonicalize(working_directory)
        .map_err(|e| format!("could not turn {working_directory:?} into a real path: {e}").into())
}

#[derive(StructOpt)]
pub enum Command {
    #[structopt(about = "deploy a shuttle project")]
    Deploy(DeployArgs),
    #[structopt(about = "view the status of a shuttle project")]
    Status,
    #[structopt(about = "view the logs of a shuttle project")]
    Logs,
    #[structopt(about = "delete the latest deployment for a shuttle project")]
    Delete,
    #[structopt(about = "create user credentials for the shuttle platform")]
    Auth(AuthArgs),
    #[structopt(about = "login to the shuttle platform")]
    Login(LoginArgs),
    #[structopt(about = "run a shuttle project locally")]
    Run(RunArgs),
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
pub struct RunArgs {
    #[structopt(long, about = "port to start service on", default_value = "8000")]
    pub port: u16,
}
