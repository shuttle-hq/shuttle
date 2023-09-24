use std::{
    fmt::Write,
    fs::read_to_string,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use cargo_generate::{GenerateArgs, TemplatePath, Vcs};
use git2::Repository;
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
    edit_shuttle_toml(&path).with_context(|| "Failed to edit Shuttle.toml.")?;
    create_gitignore_file(&path).with_context(|| "Failed to create .gitignore file.")?;

    Ok(())
}

fn set_crate_name(path: &Path, name: &str) -> Result<()> {
    let path = path.join("Cargo.toml");
    let toml_str = read_to_string(&path)?;
    let mut doc = toml_str.parse::<Document>()?;

    // change the name
    doc["package"]["name"] = value(name);

    // write the file back out
    std::fs::write(&path, doc.to_string())?;

    Ok(())
}

/// The Shuttle.toml project name override will already be in use,
/// so that property file is disruptive to a newly cloned project.
fn edit_shuttle_toml(path: &Path) -> Result<()> {
    let path = path.join("Shuttle.toml");
    if !path.exists() {
        // Do nothing if template has no Shuttle.toml
        return Ok(());
    }
    let toml_str = read_to_string(&path)?;
    let mut doc = toml_str.parse::<Document>()?;

    // remove the name
    doc.remove("name");

    if doc.len() == 0 {
        // if name was the only property in the doc, delete the file
        let _ = std::fs::remove_file(&path);

        return Ok(());
    }

    // write the file back out
    std::fs::write(&path, doc.to_string())?;

    Ok(())
}

fn create_gitignore_file(path: &Path) -> Result<()> {
    let path = path.join(".gitignore");
    let mut contents = std::fs::read_to_string(&path).unwrap_or_default();

    for rule in ["/target", ".shuttle-storage", "Secrets*.toml"] {
        if !contents.lines().any(|l| l == rule) {
            writeln!(&mut contents, "{rule}")?;
        }
    }

    std::fs::write(&path, contents)?;

    Ok(())
}
