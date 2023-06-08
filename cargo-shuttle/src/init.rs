use std::{
    fs::{read_to_string, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs};
use indoc::indoc;
use shuttle_common::project::ProjectName;
use toml_edit::{value, Document};

pub fn cargo_generate(
    path: PathBuf,
    name: &ProjectName,
    git: String,
    git_path: Option<String>,
) -> Result<()> {
    println!(r#"    Creating project "{name}" in {path:?}"#);
    let generate_args = GenerateArgs {
        init: true,
        template_path: TemplatePath {
            git: Some(git),
            auto_path: git_path,
            ..Default::default()
        },
        name: Some(name.to_string()), // appears to do nothing...
        destination: Some(path.clone()),
        vcs: Some(Vcs::Git),
        ..Default::default()
    };
    cargo_generate::generate(generate_args)
        .with_context(|| "Failed to initialize with cargo generate.")?;

    set_crate_name(&path, name.as_str()).with_context(|| "Failed to set crate name.")?;
    remove_shuttle_toml(&path);
    create_gitignore_file(&path).with_context(|| "Failed to create .gitignore file.")?;

    remove_shuttle_toml(&path);

    Ok(())
}

// since I can't get cargo-generate to do this for me...
fn set_crate_name(path: &Path, name: &str) -> Result<()> {
    // read the Cargo.toml file
    let mut path = path.to_path_buf();
    path.push("Cargo.toml");

    let toml_str = read_to_string(&path)?;
    let mut doc = toml_str.parse::<Document>()?;

    // change the name
    doc["package"]["name"] = value(name);

    // write the Cargo.toml file back out
    std::fs::write(&path, doc.to_string())?;

    Ok(())
}

/*
Currently Shuttle.toml only has a project name override.
This project name will already be in use, so the file is useless.

If we start putting more things in Shuttle.toml we may wish to re-evaluate.
*/
fn remove_shuttle_toml(path: &Path) {
    let mut path = path.to_path_buf();
    path.push("Shuttle.toml");

    // this file only exists for some of the examples, it's fine if we don't find it
    _ = std::fs::remove_file(path);
}

fn create_gitignore_file(path: &Path) -> Result<()> {
    let mut path = path.to_path_buf();
    path.push(".gitignore");

    let mut file = match OpenOptions::new().create_new(true).write(true).open(path) {
        Ok(f) => f,
        Err(e) => {
            match e.kind() {
                ErrorKind::AlreadyExists => {
                    // if the example already has a .gitignore file, just use that
                    return Ok(());
                }
                _ => {
                    return Err(anyhow!(e));
                }
            }
        }
    };

    file.write_all(indoc! {b"
        /target
        Secrets*.toml
    "})?;

    Ok(())
}
