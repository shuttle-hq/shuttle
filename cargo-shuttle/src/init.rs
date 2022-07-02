use std::fmt;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use cargo::ops::NewOptions;
use cargo_edit::{find, get_latest_dependency, registry_url};
use crate::args::InitArgs;
use toml_edit::{value, Document, Item, Table};


#[derive(Debug, Copy, Clone)]
pub enum Framework {
    Axum,
    Rocket,
    Default,
}

#[derive(Debug, Copy, Clone)]
pub enum Feature {
    Axum,
    Rocket,
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Feature::Axum => write!(f, "web-axum"),
            Feature::Rocket => write!(f, "web-rocket"),
        }
    }
}

pub struct ShuttleInit {
    pub cargo_doc: Document,
    pub cargo_toml_path: PathBuf,
    pub project_path: PathBuf,
    pub framework: Framework,
}

impl ShuttleInit {
    pub fn new(path: PathBuf, framework: Framework) -> Self {
        ShuttleInit::cargo_init(path.clone()).unwrap();

        let project_path = path;
        let cargo_toml_path = project_path.join("Cargo.toml");
        let mut cargo_doc = read_to_string(cargo_toml_path.clone()).unwrap().parse::<Document>().unwrap();
        
        // Remove empty dependencies table to re-insert after the lib table is inserted
        cargo_doc.remove("dependencies");

        // Create an empty `[lib]` table
        cargo_doc["lib"] = Item::Table(Table::new());

        Self {
            cargo_doc,
            cargo_toml_path,
            project_path,
            framework,
        }
    }

    pub fn cargo_init(path: PathBuf) -> Result<()> {
        let opts = NewOptions::new(None, false, true, path, None, None, None)?;
        let cargo_config = cargo::util::config::Config::default()?;
        let init_result = cargo::ops::init(&opts, &cargo_config)?;

        // Mimick `cargo init` behavior and log status or error to shell
        cargo_config
            .shell()
            .status("Created", format!("{} (shuttle) package", init_result))?;

        Ok(())
    }

    pub fn generate_files(&mut self) -> Result<()> {
        // Fetch the latest shuttle-service version from crates.io
        let manifest_path = find(Some(&self.project_path)).unwrap();
        let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");
        let latest_shuttle_service =
            get_latest_dependency("shuttle-service", false, &manifest_path, Some(&url))
                .expect("Could not query the latest version of shuttle-service");
        let shuttle_version = latest_shuttle_service
            .version()
            .expect("No latest shuttle-service version available");

        // Insert shuttle-service to `[dependencies]` table
        let mut dep_table = Table::new();
        dep_table["shuttle-service"]["version"] = value(shuttle_version);
        self.cargo_doc["dependencies"] = Item::Table(dep_table);

        // Truncate Cargo.toml and write the updated `Document` to it
        let mut cargo_toml = File::create(self.cargo_toml_path.clone())?;
        cargo_toml.write_all(self.cargo_doc.to_string().as_bytes())?;

        Ok(())
    }
}

pub fn get_framework(init_args: &InitArgs) -> Framework {
    if init_args.axum {
        return Framework::Axum;
    }

    if init_args.rocket {
        return Framework::Rocket;
    }

    Framework::Default
}
