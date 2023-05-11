use super::super::error::*;

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, io, path::*};
use url::Url;

/// Authentication info stored in filesystem
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoredAuth {
    auths: HashMap<String, Auth>,
}

impl StoredAuth {
    /// Load authentication info with docker and podman setting
    pub fn load_all() -> Result<Self> {
        let mut auth = StoredAuth::default();
        if let Some(path) = docker_auth_path() {
            if let Ok(new) = Self::from_path(&path) {
                auth.append(new)?;
            }
        }
        if let Some(path) = podman_auth_path() {
            if let Ok(new) = Self::from_path(&path) {
                auth.append(new)?;
            }
        }
        if let Some(path) = auth_path() {
            let new = Self::from_path(&path)?;
            auth.append(new)?;
        }
        Ok(auth)
    }

    /// Get token based on WWW-Authentication header
    pub fn challenge(&self, challenge: &AuthChallenge) -> Result<String> {
        let token_url = Url::parse(&challenge.url)?;
        let domain = token_url
            .domain()
            .expect("www-authenticate header returns invalid URL");

        let mut req = ureq::get(token_url.as_str()).set("Accept", "application/json");
        if let Some(auth) = self.auths.get(domain) {
            req = req.set("Authorization", &format!("Basic {}", auth.auth))
        }
        req = req
            .query("scope", &challenge.scope)
            .query("service", &challenge.service);
        match req.call() {
            Ok(res) => {
                let token = res.into_json::<Token>()?;
                Ok(token.token)
            }
            Err(ureq::Error::Status(..)) => Err(Error::AuthorizationFailed(token_url.to_string())),
            Err(ureq::Error::Transport(e)) => Err(Error::Network(e.to_string())),
        }
    }

    pub fn append(&mut self, other: Self) -> Result<()> {
        for (key, value) in other.auths.into_iter() {
            self.auths.insert(key, value);
        }
        Ok(())
    }

    fn from_path(path: &Path) -> Result<Self> {
        if path.is_file() {
            let f = fs::File::open(path)?;
            Ok(serde_json::from_reader(io::BufReader::new(f))?)
        } else {
            Ok(Self::default())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Auth {
    auth: String,
}

/// WWW-Authentication challenge
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthChallenge {
    pub url: String,
    pub service: String,
    pub scope: String,
}

impl AuthChallenge {
    pub fn from_header(header: &str) -> Result<Self> {
        let err = || Error::UnSupportedAuthHeader(header.to_string());
        let (ty, realm) = header.split_once(' ').ok_or_else(err)?;
        if ty != "Bearer" {
            return Err(err());
        }

        let mut url = None;
        let mut service = None;
        let mut scope = None;
        for param in realm.split(',') {
            let (key, value) = param.split_once('=').ok_or_else(err)?;
            let value = value.trim_matches('"').to_string();
            match key {
                "realm" => url = Some(value),
                "service" => service = Some(value),
                "scope" => scope = Some(value),
                _ => continue,
            }
        }
        Ok(Self {
            url: url.ok_or_else(err)?,
            service: service.ok_or_else(err)?,
            scope: scope.ok_or_else(err)?,
        })
    }
}

#[derive(Deserialize)]
struct Token {
    token: String,
}

fn auth_path() -> Option<PathBuf> {
    dirs::runtime_dir().map_or_else(
        || {
            // Most of the containers does not set XDG_RUNTIME_DIR,
            // and then this fallback to `~/.shuttle-builder/config.json` like docker.
            Some(dirs::home_dir()?.join(".shuttle-builder/config.json"))
        },
        |dir| Some(dir.join("containers/auth.json")),
    )
}

fn docker_auth_path() -> Option<PathBuf> {
    Some(dirs::home_dir()?.join(".docker/config.json"))
}

fn podman_auth_path() -> Option<PathBuf> {
    Some(dirs::runtime_dir()?.join("containers/auth.json"))
}

mod tests {}
