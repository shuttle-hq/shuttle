use std::{fs, path::Path};

use anyhow::{bail, Result};

use crate::{CommandOutput, ShuttleArgs};

pub fn resolve_pyproject_name(dir: impl AsRef<Path>) -> Result<String> {
    let pyp_path = dir.as_ref().join("pyproject.toml");
    if !(pyp_path.exists() && pyp_path.is_file()) {
        bail!("No pyproject.toml found in directory");
    }
    let doc: toml_edit::DocumentMut = fs::read_to_string(pyp_path).unwrap().parse().unwrap();
    let project_name = doc["project"]["name"].as_str().unwrap();

    Ok(project_name.to_owned())
}

pub fn impulse_build(args: ShuttleArgs) -> Result<CommandOutput> {
    let project_name = resolve_pyproject_name(args.project_args.working_directory.as_path())?;

    if !std::process::Command::new(/* nixpacks_binary.as_deref().unwrap_or( */ "nixpacks")
        .arg("--version")
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
    {
        bail!("nixpacks binary not found");
    }

    if !std::process::Command::new(/* nixpacks_binary.as_deref().unwrap_or( */ "nixpacks")
        .arg("build")
        .arg("--name")
        .arg(project_name)
        .arg(args.project_args.working_directory)
        .spawn()
        .unwrap()
        .wait()
        .unwrap()
        .success()
    {
        bail!("nixpacks build failed");
    }

    Ok(CommandOutput::None)
}

pub fn impulse_push(args: ShuttleArgs) -> Result<()> {
    impulse_build(args)?;
    let project_name = resolve_pyproject_name(args.project_args.working_directory.as_path())?;

    let docker_host = todo!();
    let image_url = format!("{docker_host}/{project_name}/something:latest");

    Ok(())
}
