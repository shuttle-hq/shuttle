use std::collections::{hash_map::DefaultHasher, HashSet};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bollard::{auth::DockerCredentials, Docker};
use futures::TryStreamExt;
use ignore::DirEntry;
use ignore::WalkBuilder;
use impulse_common::types::auth::RegistryAuthResponse;
use nixpacks::nixpacks::{
    builder::docker::DockerBuilderOptions,
    plan::{generator::GeneratePlanOptions, phase::StartPhase, BuildPlan},
};
use std::io::Read;
use tar::Builder;
use zip::write::FileOptions;
use zip::ZipWriter;

use crate::{args::DeployArgs, Neptune};

pub(crate) enum ArchiveType {
    Tar,
    Zip,
}
enum ArchiverType<'a, W: Write + Seek> {
    Tar(Builder<&'a mut W>),
    Zip(ZipWriter<&'a mut W>),
}
struct Archiver<'a, W: Write + Seek> {
    inner: ArchiverType<'a, W>,
}

impl<'a, W: Write + Seek> Archiver<'a, W> {
    fn tar(manifest_data: &'a mut W) -> Self {
        Archiver {
            inner: ArchiverType::Tar(Builder::new(manifest_data)),
        }
    }

    fn zip(manifest_data: &'a mut W) -> Self {
        Archiver {
            inner: ArchiverType::Zip(ZipWriter::new(manifest_data)),
        }
    }

    fn add_file(
        &mut self,
        path: impl AsRef<Path>,
        rel_path: impl AsRef<Path>,
    ) -> std::result::Result<(), std::io::Error> {
        let path = path.as_ref();
        let rel_path = rel_path.as_ref();
        match &mut self.inner {
            ArchiverType::Tar(ref mut manifest) => manifest.append_path_with_name(path, rel_path),
            ArchiverType::Zip(ref mut manifest) => {
                let options: FileOptions<'_, zip::write::ExtendedFileOptions> =
                    FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
                manifest.start_file_from_path(rel_path, options)?;
                let mut f = std::fs::File::open(path)?;
                std::io::copy(&mut f, manifest)?;
                Ok(())
            }
        }
    }

    fn finish(self) -> std::result::Result<(), std::io::Error> {
        match self.inner {
            ArchiverType::Tar(mut manifest) => manifest.finish(),
            ArchiverType::Zip(manifest) => {
                manifest.finish()?;
                Ok(())
            }
        }
    }
}

impl Neptune {
    pub(crate) fn create_build_context(
        &self,
        context_root: impl AsRef<Path>,
        archive_type: ArchiveType, // dockerfile_filename: &Path,
        ignore_files: Option<Vec<impl AsRef<Path>>>,
        standard_filters: bool,
    ) -> Result<Vec<u8>> {
        let root = context_root.as_ref();
        let mut manifest_data = Cursor::new(Vec::new());
        {
            let mut manifest = match archive_type {
                ArchiveType::Tar => Archiver::tar(&mut manifest_data),
                ArchiveType::Zip => Archiver::zip(&mut manifest_data),
            };

            let mut walk_builder = WalkBuilder::new(root);

            if let Some(files) = ignore_files {
                for file in files {
                    if let Some(err) = walk_builder.add_ignore(file) {
                        return Err(err.into());
                    }
                }
            }

            if standard_filters {
                tracing::debug!("Setting walk filter standard filters (.gitignore)",);
                walk_builder.standard_filters(standard_filters);
            }

            tracing::debug!("Creating build context archive from {:?}", root);

            let mut total_size: u64 = 0;
            let mut included_paths: HashSet<PathBuf> = HashSet::new();
            const MAX_ARCHIVE_SIZE: u64 = 200 * 1024 * 1024; // 200MB in bytes

            for entry in walk_builder.build() {
                let entry: DirEntry = entry?;
                let path = entry.path();

                if path.is_file() {
                    // Check file size and accumulate total size
                    let file_size = fs::metadata(path)?.len();
                    total_size += file_size;

                    if total_size > MAX_ARCHIVE_SIZE {
                        return Err(anyhow::anyhow!(
                            "Archive size would exceed 200MB limit. Current size: {} bytes",
                            total_size
                        ));
                    }

                    let rel_path = path.strip_prefix(root)?;
                    tracing::debug!("Adding entry: {:?}", rel_path);
                    manifest.add_file(path, rel_path)?;
                    included_paths.insert(rel_path.to_path_buf());
                }
            }

            const LINT_CONFIG_FILES: [&str; 2] = [".neptune-lint.toml", "neptune-lint.toml"];
            for lint_file in LINT_CONFIG_FILES {
                let abs_path = root.join(lint_file);
                if abs_path.is_file() {
                    let rel_path = PathBuf::from(lint_file);
                    if !included_paths.contains(&rel_path) {
                        manifest.add_file(&abs_path, &rel_path)?;
                    }
                }
            }

            manifest.finish()?;
        }

        Ok(manifest_data.into_inner())
    }

    pub async fn build(&self, project_id: &str, build_args: DeployArgs) -> Result<Option<String>> {
        let wd = &self.global_args.working_directory;

        let docker = Docker::connect_with_local_defaults()?;

        let image_name = if let Some(tag) = build_args.tag {
            tag
        } else {
            self.global_args
                .workdir_name()
                .context("getting name of working directory")?
        };

        // Generate unique tag
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut hasher = DefaultHasher::new();
        timestamp.hash(&mut hasher);
        let unique_tag = format!("{:x}", hasher.finish())[..12].to_string();

        // Fetch a registry token
        let RegistryAuthResponse { token, url } =
            self.client.registry_auth(project_id).await?.into_inner();

        tracing::debug!("Registry auth token: {}", token);

        let image_with_tag = format!("{}:{}", &url, unique_tag);

        if tokio::fs::try_exists("Dockerfile").await? {
            tracing::info!("Using local Dockerfile...");
            let tar: Vec<u8> = self.create_build_context(
                wd,
                ArchiveType::Tar,
                Some(vec![Path::new(".dockerfile")]),
                false,
            )?;
            let mut logs = vec![];
            match docker
                .build_image(
                    bollard::image::BuildImageOptions {
                        dockerfile: String::from("Dockerfile"),
                        t: String::from(&image_with_tag),
                        ..Default::default()
                    },
                    None,
                    Some(tar.into()),
                )
                .map_ok(|log| match log {
                    bollard::models::BuildInfo { error: Some(e), .. } => {
                        eprintln!("{}", e);
                    }
                    bollard::models::BuildInfo {
                        stream: Some(v), ..
                    } => {
                        tracing::info!("{}", &v);
                        logs.push(v);
                    }
                    _ => {}
                })
                .try_collect::<Vec<_>>()
                .await
            {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Build error: {e:?}");
                    for line in logs {
                        eprintln!("{}", line);
                    }
                    return Ok(None);
                }
            }
        } else {
            // nixpacks takes in a relative path str which is then canonicalized...
            // so calculate the relative path of wd from cwd
            let rel_path = wd
                .strip_prefix(&env::current_dir().unwrap())
                .unwrap()
                .to_str()
                .unwrap();

            // Read start command generated by `neptune generate spec` if present
            let start_command = fs::read_to_string(
                self.global_args
                    .working_directory
                    .join(".neptune")
                    .join("start_command"),
            )
            .ok()
            .map(|s| s.trim().to_string());
            let mut build_plan = BuildPlan::default();
            if let Some(cmd) = start_command {
                if !cmd.is_empty() {
                    build_plan.set_start_phase(StartPhase::new(cmd));
                }
            }

            // If emitting, capture stdout while nixpacks prints the Dockerfile
            let dockerfile_path = self.global_args.working_directory.join("Dockerfile");
            let want_emit = build_args.emit_dockerfile && !dockerfile_path.exists();
            if want_emit {
                let mut redirect = gag::BufferRedirect::stdout()?;
                let res = nixpacks::create_docker_image(
                    rel_path,
                    build_args.env.iter().map(|e| e.as_str()).collect(),
                    &GeneratePlanOptions {
                        plan: Some(build_plan),
                        config_file: None,
                    },
                    &DockerBuilderOptions {
                        name: Some(image_name.clone()),
                        print_dockerfile: true,
                        platform: vec![String::from("linux/amd64")],
                        tags: vec![String::from(&unique_tag)],
                        no_error_without_start: true,
                        ..Default::default()
                    },
                )
                .await;
                let mut captured = String::new();
                redirect.read_to_string(&mut captured)?;
                res?;

                // Extract Dockerfile content heuristically from captured output
                let mut start_idx = None;
                let mut collected = Vec::new();
                for (i, line) in captured.lines().enumerate() {
                    if line.trim_start().starts_with("FROM ") {
                        start_idx = Some(i);
                        break;
                    }
                }
                if let Some(start) = start_idx {
                    collected.extend(captured.lines().skip(start));
                    let dockerfile_text = collected.join("\n");
                    if !dockerfile_text.trim().is_empty() {
                        fs::write(&dockerfile_path, dockerfile_text)?;
                        tracing::info!("Wrote generated Dockerfile to {:?}", dockerfile_path);
                    } else {
                        tracing::warn!("Nixpacks printed no Dockerfile content; skipping write");
                    }
                } else {
                    tracing::warn!(
                        "Could not detect Dockerfile in nixpacks output; skipping write"
                    );
                }
            } else {
                nixpacks::create_docker_image(
                    rel_path,
                    build_args.env.iter().map(|e| e.as_str()).collect(),
                    &GeneratePlanOptions {
                        plan: Some(build_plan),
                        config_file: None,
                    },
                    &DockerBuilderOptions {
                        name: Some(image_name.clone()),
                        print_dockerfile: build_args.emit_dockerfile,
                        tags: vec![String::from(&unique_tag)],
                        no_error_without_start: true,
                        ..Default::default()
                    },
                )
                .await?;
            }
        }

        docker
            .tag_image(
                &image_name,
                Some(bollard::image::TagImageOptions {
                    repo: String::from(&url),
                    tag: String::from(&unique_tag),
                }),
            )
            .await?;

        let inspect = docker.inspect_image(&image_name).await?;
        let image_with_digest =
            if let Some(&[digest, ..]) = inspect.repo_digests.as_deref().as_ref() {
                digest
            } else {
                &image_with_tag
            };

        if let Err(e) = docker
            .push_image(
                &image_with_tag,
                Some(bollard::image::PushImageOptions {
                    tag: String::from(unique_tag),
                }),
                Some(DockerCredentials {
                    username: Some(String::from("_token")),
                    password: Some(token),
                    serveraddress: Some(String::from("europe-west2-docker.pkg.dev")),
                    ..Default::default()
                }),
            )
            .try_collect::<Vec<_>>()
            .await
        {
            match e {
                bollard::errors::Error::DockerStreamError { error, .. } => {
                    return Err(anyhow::anyhow!("Docker push failed: {}", error));
                }
                bollard::errors::Error::DockerResponseServerError {
                    status_code,
                    message,
                } => {
                    return Err(anyhow::anyhow!(
                        "Docker push failed (status {}): {}",
                        status_code,
                        message
                    ));
                }
                other => {
                    return Err(other.into());
                }
            }
        }

        Ok(Some(String::from(image_with_digest)))
    }
}
