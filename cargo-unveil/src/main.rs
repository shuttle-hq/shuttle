mod args;
mod client;
mod config;

use crate::args::{Args, DeleteArgs, DeployArgs, StatusArgs};
use anyhow::{Context, Result};
use cargo::core::resolver::CliFeatures;
use cargo::core::Workspace;
use cargo::ops::{PackageOpts, Packages};
use std::env;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use structopt::StructOpt;

fn main() -> Result<()> {
    let args: Args = Args::from_args();
    match args {
        Args::Deploy(deploy_args) => deploy(deploy_args),
        Args::Status(status_args) => status(status_args),
        Args::Delete(delete_args) => delete(delete_args),
    }
}

fn delete(args: DeleteArgs) -> Result<()> {
    let api_key = config::get_api_key().context("failed to retrieve api key")?;
    client::delete(api_key, args.deployment_id).context("failed to delete deployment")
}

fn status(args: StatusArgs) -> Result<()> {
    let api_key = config::get_api_key().context("failed to retrieve api key")?;
    client::status(api_key, args.deployment_id).context("failed to get status of deployment")
}

fn deploy(args: DeployArgs) -> Result<()> {
    let working_directory = env::current_dir()?;
    let api_key = config::get_api_key().context("failed to retrieve api key")?;
    let project = config::get_project(&working_directory)
        .context("failed to retrieve project configuration")?;
    let package_file = run_cargo_package(&working_directory, args.allow_dirty)
        .context("failed to package cargo project")?;
    client::deploy(package_file, api_key, project).context("failed to deploy cargo project")
}

// Packages the cargo project and returns a File to that file
fn run_cargo_package(working_directory: &Path, allow_dirty: bool) -> Result<File> {
    let config = cargo::util::config::Config::default()?;
    let path = working_directory.join("Cargo.toml");

    let ws = Workspace::new(&path, &config)?;
    let opts = PackageOpts {
        config: &config,
        list: false,
        check_metadata: true,
        allow_dirty,
        verify: false,
        jobs: None,
        to_package: Packages::Default,
        targets: vec![],
        cli_features: CliFeatures {
            features: Rc::new(Default::default()),
            all_features: false,
            uses_default_features: true,
        },
    };

    let locks = cargo::ops::package(&ws, &opts)?.expect("unwrap ok here");
    let owned = locks.get(0).unwrap().file().try_clone()?;
    Ok(owned)
}
