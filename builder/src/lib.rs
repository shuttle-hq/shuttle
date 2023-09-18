use std::{
    collections::BTreeMap,
    fs::{self, remove_file},
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
};

use async_trait::async_trait;
use flate2::read::GzDecoder;
use nbuild_core::models::{cargo, nix};
use shuttle_common::{backends::auth::VerifyClaim, claims::Scope};
use shuttle_proto::builder::{
    build_response::Secret, builder_server::Builder, BuildRequest, BuildResponse,
};
use tar::Archive;
use tempfile::tempdir;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument};

pub mod args;

/// A wrapper to capture any error possible with this service
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("build error: {0}")]
    Build(#[from] nbuild_core::Error),

    #[error("error reading secrets: {0}")]
    Secrets(#[from] toml::de::Error),
}

#[derive(Default)]
pub struct Service;

impl Service {
    pub fn new() -> Self {
        Self
    }

    #[instrument(name = "Building deployment", skip(self, archive))]
    async fn build(
        &self,
        deployment_id: String,
        archive: Vec<u8>,
    ) -> Result<(Vec<u8>, BTreeMap<String, String>), Error> {
        let tmp_dir = tempdir()?;
        let path = tmp_dir.path();

        extract_tar_gz_data(archive.as_slice(), path).await?;
        let secrets = get_secrets(path).await?;
        build_flake_file(path)?;

        let mut cmd = Command::new("nix");
        let output_path = path.join("_archive");
        cmd.args([
            "build",
            "--no-write-lock-file",
            "--impure",
            "--log-format",
            "bar-with-logs",
            "--out-link",
            output_path.to_str().unwrap(),
            path.to_str().unwrap(),
        ])
        .stdout(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("to get handle on stdout");

        let mut reader = BufReader::new(stdout).lines();

        tokio::spawn(async move {
            let id = deployment_id.clone();
            while let Some(line) = reader.next_line().await.expect("to get line") {
                info!(deployment_id = %id, "{line}");
            }
        });

        let status = child.wait().await.expect("build to finish");

        debug!(deployment_id, "{status}");

        let archive = fs::read(output_path)?;

        Ok((archive, secrets))
    }
}

#[async_trait]
impl Builder for Service {
    async fn build(
        &self,
        request: Request<BuildRequest>,
    ) -> Result<Response<BuildResponse>, Status> {
        request.verify(Scope::DeploymentPush)?;

        let BuildRequest {
            deployment_id,
            archive,
        } = request.into_inner();
        let (image, secrets) = match self.build(deployment_id, archive).await {
            Ok(results) => results,
            Err(error) => {
                error!(
                    error = &error as &dyn std::error::Error,
                    "failed to build image"
                );

                return Err(Status::from_error(Box::new(error)));
            }
        };

        let secrets = secrets
            .into_iter()
            .map(|(key, value)| Secret { key, value })
            .collect();

        let result = BuildResponse {
            image,
            is_wasm: false,
            secrets,
        };

        Ok(Response::new(result))
    }
}

/// Equivalent to the command: `tar -xzf --strip-components 1`
#[instrument(skip(data, dest))]
async fn extract_tar_gz_data(data: impl Read, dest: impl AsRef<Path>) -> Result<(), Error> {
    let tar = GzDecoder::new(data);
    let mut archive = Archive::new(tar);
    archive.set_overwrite(true);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path: PathBuf = entry.path()?.components().skip(1).collect();
        let dst: PathBuf = dest.as_ref().join(path);
        std::fs::create_dir_all(dst.parent().unwrap())?;
        entry.unpack(dst)?;
    }

    Ok(())
}

/// Make a `flake.nix` file at the given path
fn build_flake_file(path: &Path) -> Result<(), Error> {
    let mut package = cargo::Package::from_current_dir(path)?;
    package.resolve();

    let package: nix::Package = package.into();
    let name = package.name().to_string();
    let expr = package.into_derivative();

    fs::write(path.join(".nbuild.nix"), expr)?;

    let flake = format!(
        r#"{{
  inputs = {{
    rust-overlay.url = "github:oxalica/rust-overlay";
  }};
  outputs = {{ self, nixpkgs, flake-utils, rust-overlay, ... }}:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs{{ inherit system overlays; }};
      in {{
        packages = rec {{
          runtime = (import ./.nbuild.nix {{ inherit pkgs; }});
          default = pkgs.dockerTools.buildLayeredImage {{
            name = "{name}-runtime";
            config = {{ Entrypoint = [ "${{runtime}}/bin/{name}" ]; }};
          }};
        }};
      }}
    );
}}"#
    );

    fs::write(path.join("flake.nix"), flake)?;

    Ok(())
}

/// Get secrets from `Secrets.toml`
async fn get_secrets(path: &Path) -> Result<BTreeMap<String, String>, Error> {
    let secrets_file = path.join("Secrets.toml");

    if secrets_file.exists() && secrets_file.is_file() {
        let secrets_str = tokio::fs::read_to_string(secrets_file.clone()).await?;

        let secrets: BTreeMap<String, String> = secrets_str.parse::<toml::Value>()?.try_into()?;

        remove_file(secrets_file)?;

        Ok(secrets)
    } else {
        Ok(Default::default())
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs::File, io::Write};

    use tempfile::Builder;
    use tokio::fs;

    #[tokio::test]
    async fn extract_tar_gz_data() {
        let dir = Builder::new()
            .prefix("shuttle-extraction-test")
            .tempdir()
            .unwrap();
        let p = dir.path();

        // Binary data for an archive in the following form:
        //
        // - temp
        //   - world.txt
        //   - subdir
        //     - hello.txt
        let test_data = hex::decode(
            "\
1f8b0800000000000003edd5d10a823014c6f15df7143e41ede8997b1e4d\
a3c03074528f9f0a41755174b1a2faff6e0653d8818f7d0bf5feb03271d9\
91f76e5ac53b7bbd5e18d1d4a96a96e6a9b16225f7267191e79a0d7d28ba\
2431fbe2f4f0bf67dfbf5498f23fb65d532dc329c439630a38cff541fe7a\
977f6a9d98c4c619e7d69fe75f94ebc5a767c0e7ccf7bf1fca6ad7457b06\
5eea7f95f1fe8b3aa5ffdfe13aff6ddd346d8467e0a5fef7e3be649928fd\
ff0e55bda1ff01000000000000000000e0079c01ff12a55500280000",
        )
        .unwrap();

        super::extract_tar_gz_data(test_data.as_slice(), &p)
            .await
            .unwrap();
        assert!(fs::read_to_string(p.join("world.txt"))
            .await
            .unwrap()
            .starts_with("abc"));
        assert!(fs::read_to_string(p.join("subdir/hello.txt"))
            .await
            .unwrap()
            .starts_with("def"));
    }

    #[tokio::test]
    async fn get_secrets() {
        let temp = Builder::new().prefix("secrets").tempdir().unwrap();
        let temp_p = temp.path();

        let secret_p = temp_p.join("Secrets.toml");
        let mut secret_file = File::create(secret_p.clone()).unwrap();
        secret_file.write_all(b"KEY = 'value'").unwrap();

        let actual = super::get_secrets(temp_p).await.unwrap();
        let expected = BTreeMap::from([("KEY".to_string(), "value".to_string())]);

        assert_eq!(actual, expected);

        assert!(!secret_p.exists(), "the secrets file should be deleted");
    }
}
