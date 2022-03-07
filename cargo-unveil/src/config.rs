use lib::ApiKey;
use serde::Deserialize;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use lib::project::ProjectConfig;

/// Global client config for things like API keys.
#[derive(Deserialize)]
pub(crate) struct Config {
    api_key: ApiKey,
}

pub(crate) fn get_api_key() -> Result<ApiKey> {
    let mut directory = unveil_config_dir()?;
    let file_path = unveil_config_file(&mut directory);
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

fn unveil_config_file(path: &mut PathBuf) -> PathBuf {
    path.join("config.toml")
}

fn unveil_config_dir() -> Result<PathBuf> {
    let unveil_config_dir = dirs::config_dir().ok_or_else(|| {
        anyhow!(
            "Could not find a configuration directory. Your operating system may not be supported."
        )
    })?;
    Ok(unveil_config_dir.join("unveil"))
}

pub(crate) fn get_project(working_directory: &Path) -> Result<Option<ProjectConfig>> {
    let project_config_path = working_directory.join("Unveil.toml");
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
