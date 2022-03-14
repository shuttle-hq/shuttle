use shuttle_common::ApiKey;
use serde::{Deserialize, Serialize};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use shuttle_common::project::ProjectConfig;

/// Global client config for things like API keys.
#[derive(Deserialize, Serialize)]
pub(crate) struct Config {
    api_key: ApiKey,
}

pub(crate) fn create_with_api_key(api_key: String) -> Result<()> {
    let mut directory = shuttle_config_dir()?;
    std::fs::create_dir_all(&directory)?;

    let file_path = shuttle_config_file(&mut directory);
    let mut config_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(file_path)?;

    let config = Config {
        api_key
    };

    Ok(write!(config_file, "{}", toml::to_string_pretty(&config)?)?)
}

pub(crate) fn config_file_exists() -> Result<bool> {
    let mut directory = shuttle_config_dir()?;
    let file_path = shuttle_config_file(&mut directory);
    Ok(file_path.exists())
}

pub(crate) fn get_api_key() -> Result<ApiKey> {
    let mut directory = shuttle_config_dir()?;
    let file_path = shuttle_config_file(&mut directory);
    let file_contents: String = match std::fs::read_to_string(file_path) {
        Ok(file_contents) => Ok(file_contents),
        Err(e) => match e.kind() {
            ErrorKind::NotFound => Err(anyhow!("could not find `config.toml` in {:?}", directory)),
            _ => Err(e.into()),
        },
    }?;
    let config: Config = toml::from_str(&file_contents)?;
    Ok(config.api_key)
}

fn shuttle_config_file(path: &mut PathBuf) -> PathBuf {
    path.join("config.toml")
}

fn shuttle_config_dir() -> Result<PathBuf> {
    let shuttle_config_dir = dirs::config_dir().ok_or_else(|| {
        anyhow!(
            "Could not find a configuration directory. Your operating system may not be supported."
        )
    })?;
    Ok(shuttle_config_dir.join("shuttle"))
}

pub(crate) fn get_project(working_directory: &Path) -> Result<Option<ProjectConfig>> {
    let project_config_path = working_directory.join("Shuttle.toml");
    let file_contents: String = match std::fs::read_to_string(project_config_path) {
        Ok(file_contents) => file_contents,
        Err(e) => {
            return match e.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(e.into()),
            }
        }
    };
    let project: ProjectConfig = toml::from_str(&file_contents)?;
    Ok(Some(project))
}
