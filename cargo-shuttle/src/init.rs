use std::{
    fs::{read_to_string, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Context, Result};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs};
use git2::Repository;
use indoc::indoc;
use shuttle_common::project::ProjectName;
use toml_edit::{value, Document};

use crate::args::TemplateLocation;

/// More about how this works: https://cargo-generate.github.io/cargo-generate/
pub fn cargo_generate(path: PathBuf, name: &ProjectName, temp_loc: TemplateLocation) -> Result<()> {
    println!(r#"    Creating project "{name}" in {path:?}"#);

    let do_git_init = Repository::discover(&path).is_err();

    let generate_args = GenerateArgs {
        template_path: TemplatePath {
            // Automatically guess location from:
            // - cargo-generate "favorites", see their docs
            // - git hosts (gh:, gl: etc.)
            // - local path (check if exists)
            // - github username+repo (shuttle-hq/shuttle-examples)
            auto_path: Some(temp_loc.auto_path.clone()),
            // subfolder in the source folder that was found
            subfolder: temp_loc.subfolder,
            ..Default::default()
        },
        // setting this prevents cargo-generate from prompting the user.
        // it will then be used to try and replace a "{{project-name}}" placeholder in the cloned folder.
        // (not intended with Shuttle templates)
        name: Some(name.to_string()),
        destination: Some(path.clone()),
        init: true, // don't create a folder to place the template in
        vcs: Some(Vcs::Git),
        force_git_init: do_git_init, // git init after cloning
        ..Default::default()
    };
    cargo_generate::generate(generate_args)
        .with_context(|| "Failed to initialize with cargo generate.")?;

    set_crate_name(&path, name.as_str())
        .with_context(|| "Failed to set crate name. No Cargo.toml in template?")?;
    remove_shuttle_toml(&path);
    create_gitignore_file(&path).with_context(|| "Failed to create .gitignore file.")?;

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
