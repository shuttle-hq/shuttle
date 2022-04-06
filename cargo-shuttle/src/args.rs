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
pub enum Args {
    #[structopt(about = "deploy an shuttle project")]
    Deploy(DeployArgs),
    #[structopt(about = "view the status of an shuttle project")]
    Status(ProjectArgs),
    #[structopt(about = "delete the latest deployment for a shuttle project")]
    Delete(ProjectArgs),
    #[structopt(about = "create user credentials for the shuttle platform")]
    Auth(AuthArgs),
    #[structopt(about = "login to the shuttle platform")]
    Login(LoginArgs),
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
    #[structopt(flatten)]
    pub project_args: ProjectArgs,
}

fn parse_working_directory(working_directory: &OsStr) -> Result<PathBuf, OsString> {
    canonicalize(working_directory)
        .map_err(|e| format!("could not turn {working_directory:?} into a real path: {e}").into())
}

// Common args for subcommands that deal with projects.
#[derive(StructOpt)]
pub struct ProjectArgs {
    #[structopt(
        long,
        parse(try_from_os_str = parse_working_directory),
        default_value = ".",
        about = "specify the working directory"
    )]
    pub working_directory: PathBuf,
    #[structopt(long, about = "specify the name of the project (overrides crate name)")]
    pub name: Option<ProjectName>,
}
