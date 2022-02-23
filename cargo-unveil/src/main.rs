mod config;
mod client;


use std::env;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use anyhow::{Context, Result};
use cargo::core::resolver::CliFeatures;
use cargo::core::Workspace;
use cargo::ops::{PackageOpts, Packages};


fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    println!("{:?}",args.get(0)); // 1 is the right index
    let working_directory = env::current_dir()?;
    let api_key = config::get_api_key().context("failed to retrieve api key")?;
    let project = config::get_project(&working_directory).context("failed to retrieve project configuration")?;
    let package_file = run_cargo_package(&working_directory).context("failed to package cargo project")?;
    client::deploy(package_file, api_key, project).context("failed to deploy cargo project")?;
    Ok(())
}

// Packages the cargo project and returns a File to that file
fn run_cargo_package(working_directory: &Path) -> Result<File> {
    let config = cargo::util::config::Config::default()?;
    let path = working_directory.join("Cargo.toml");

    let ws = Workspace::new(&path, &config)?;
    let opts = PackageOpts {
        config: &config,
        list: false,
        check_metadata: true,
        allow_dirty: true, //todo
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

