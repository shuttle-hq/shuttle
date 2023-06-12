use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Stdio,
};

use async_trait::async_trait;
use flate2::read::GzDecoder;
use nbuild_core::models::{cargo, nix};
use shuttle_common::{backends::auth::VerifyClaim, claims::Scope};
use shuttle_proto::builder::{builder_server::Builder, BuildRequest, BuildResponse};
use tar::Archive;
use tempfile::tempdir;
use thiserror::Error;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use tonic::{Request, Response, Status};
use tracing::{error, instrument};

/// A wrapper to capture any error possible with this service
#[derive(Debug, Error)]
pub enum Error {
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),

    #[error("build error: {0}")]
    Build(#[from] nbuild_core::Error),
}

pub struct Service;

impl Service {
    pub fn new() -> Self {
        Self
    }

    #[instrument(skip(self, archive))]
    async fn build(&self, deployment_id: String, archive: Vec<u8>) -> Result<Vec<u8>, Error> {
        let tmp_dir = tempdir()?;

        extract_tar_gz_data(archive.as_slice(), tmp_dir.path()).await?;
        build_flake_file(tmp_dir.path())?;

        let mut cmd = Command::new("nix");
        cmd.args(["build", tmp_dir.path().to_str().unwrap()])
            .stdout(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("to get handle on stdout");

        let mut reader = BufReader::new(stdout).lines();

        tokio::spawn(async move {
            while let Some(line) = reader.next_line().await.expect("to get line") {
                println!("{line}");
            }
        });

        let status = child.wait().await.expect("build to finish");

        println!("status: {status}");

        Ok(Default::default())
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
        let image = match self.build(deployment_id, archive).await {
            Ok(image) => image,
            Err(error) => {
                error!(
                    error = &error as &dyn std::error::Error,
                    "failed to build image"
                );

                return Err(Status::from_error(Box::new(error)));
            }
        };
        let result = BuildResponse {
            image,
            is_wasm: false,
            secrets: Default::default(),
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
    // let name = package.name;
    let name = "test";
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
            config = {{ Cmd = [ "${{runtime}}/bin/{name}" ]; }};
          }};
        }};
      }}
    );
}}"#
    );

    fs::write(path.join("flake.nix"), flake)?;

    Ok(())
}
