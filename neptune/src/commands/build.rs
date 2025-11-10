use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::io::{Cursor, Seek, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bollard::{auth::DockerCredentials, Docker};
use futures::TryStreamExt;
use ignore::{gitignore::GitignoreBuilder, Match};
use nixpacks::nixpacks::{
    builder::docker::DockerBuilderOptions,
    plan::{generator::GeneratePlanOptions, phase::StartPhase, BuildPlan},
};
use tar::Builder;
use walkdir::WalkDir;
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
        match &mut self.inner {
            ArchiverType::Tar(ref mut manifest) => manifest.append_path_with_name(path, rel_path),
            ArchiverType::Zip(ref mut manifest) => {
                let options: FileOptions<'_, zip::write::ExtendedFileOptions> =
                    FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
                manifest.start_file_from_path(path, options)?;
                let mut f = std::fs::File::open(rel_path)?;
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
    fn load_dockerignore(
        &self,
        dockerfile_path: impl AsRef<Path>,
    ) -> Result<Option<ignore::gitignore::Gitignore>> {
        let dockerignore_path = dockerfile_path.as_ref().join(".dockerignore");
        if !dockerignore_path.exists() {
            return Ok(None);
        }

        let mut builder = GitignoreBuilder::new(dockerfile_path);
        builder.add(&dockerignore_path);
        Ok(Some(builder.build()?))
    }

    pub(crate) fn create_build_context(
        &self,
        context_root: impl AsRef<Path>,
        archive_type: ArchiveType, // dockerfile_filename: &Path,
    ) -> Result<Vec<u8>> {
        let mut manifest_data = Cursor::new(Vec::new());
        {
            let mut manifest = match archive_type {
                ArchiveType::Tar => Archiver::tar(&mut manifest_data),
                ArchiveType::Zip => Archiver::zip(&mut manifest_data),
            };
            let ignore_spec = self.load_dockerignore(&context_root)?;

            tracing::debug!(
                "Creating build context tar from {:?}",
                context_root.as_ref()
            );

            // TODO: safety check that we don't zip user's $HOME directory

            for entry in WalkDir::new(&context_root).into_iter() {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    let rel_path = path.strip_prefix(&context_root)?;

                    // Check against .dockerignore patterns
                    if let Some(ref ignore) = ignore_spec {
                        if let Match::Ignore(_) = ignore.matched(rel_path, false) {
                            tracing::debug!("Ignoring file due to .dockerignore: {:?}", rel_path);
                            continue;
                        }
                    }
                    tracing::debug!("Adding tar entry: {:?}", rel_path);

                    manifest.add_file(path, rel_path)?;
                }
            }

            tracing::debug!("{:?}", context_root.as_ref());

            // Add the Dockerfile to the tar
            // let dockerfile_path = context_root.join(dockerfile_filename);
            // if dockerfile_path.exists() && dockerfile_path.is_file() {
            //     let mut dockerfile_contents = Vec::new();
            //     File::open(&dockerfile_path)?.read_to_end(&mut dockerfile_contents)?;

            //     let mut header = Header::new_gnu();
            //     header.set_path("Dockerfile")?;
            //     header.set_size(dockerfile_contents.len() as u64);
            //     header.set_cksum();

            //     tar.append(&header, dockerfile_contents.as_slice())?;
            // }

            manifest.finish()?;
        }

        Ok(manifest_data.into_inner())
    }

    pub async fn build(&self, name: &str, build_args: DeployArgs) -> Result<Option<String>> {
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

        let uri = format!(
            "europe-west2-docker.pkg.dev/dev-shuttlebox/cloud-run-deploy-public/{}",
            name
        );

        let final_image_uri = format!("{}:{}", &uri, unique_tag);

        if tokio::fs::try_exists("Dockerfile").await? {
            tracing::info!("Using local Dockerfile...");
            let tar: Vec<u8> = self.create_build_context(wd, ArchiveType::Tar)?;
            let mut logs = vec![];
            match docker
                .build_image(
                    bollard::image::BuildImageOptions {
                        dockerfile: String::from("Dockerfile"),
                        t: String::from(&final_image_uri),
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

            // TODO: figure out where to pass start command from
            let start_command = "TODO";
            let mut build_plan = BuildPlan::default();
            if start_command == "TODO" {
                build_plan.set_start_phase(StartPhase::new(start_command));
            }

            nixpacks::create_docker_image(
                rel_path,
                build_args.env.iter().map(|e| e.as_str()).collect(),
                &GeneratePlanOptions {
                    plan: Some(build_plan),
                    config_file: None,
                },
                &DockerBuilderOptions {
                    name: Some(image_name.clone()),
                    // print_dockerfile: build_args.print_dockerfile,
                    tags: vec![String::from(&unique_tag)],
                    no_error_without_start: true,
                    ..Default::default()
                },
            )
            .await?;
        }

        // Fetch a registry token
        let token = self.client.registry_auth().await?;
        let token_string = String::from_utf8_lossy(&token).to_string();

        tracing::debug!("Registry auth token: {}", token_string);

        docker
            .tag_image(
                &image_name,
                Some(bollard::image::TagImageOptions {
                    repo: String::from(&uri),
                    tag: String::from(&unique_tag),
                }),
            )
            .await?;

        docker
            .push_image(
                &final_image_uri,
                Some(bollard::image::PushImageOptions {
                    tag: String::from(unique_tag),
                }),
                Some(DockerCredentials {
                    username: Some(String::from("_token")),
                    password: Some(token_string),
                    serveraddress: Some(String::from("europe-west2-docker.pkg.dev")),
                    ..Default::default()
                }),
            )
            .try_collect::<Vec<_>>()
            .await?;

        // if self.global_args.output_mode == crate::OutputMode::Json {
        //     println!(
        //         indoc::indoc! {r#"
        //         {{
        //             "ok": true,
        //             "project": "{}",
        //             "image_uri": "{}",
        //             "summary": "Image built and pushed successfully.",
        //             "messages": ["Image is ready for deployment."],
        //             "next_action": "deploy",
        //             "requires_confirmation": false,
        //             "next_action_tool": "neptune-deploy",
        //             "next_action_params": {{
        //                 "image_uri": "{}"
        //             }},
        //             "next_action_non_tool": "Run 'neptune deploy' to deploy the built image to your infrastructure."
        //         }}"#
        //         },
        //         name, final_image_uri, final_image_uri
        //     );
        // } else if self.global_args.verbose {
        //     println!(
        //         indoc::indoc! {r#"
        //         ✅ Build completed successfully!

        //         Project: {}
        //         Image URI: {}

        //         Your application has been built and pushed to the container registry.
        //         The image is now ready for deployment to your infrastructure.

        //         Next step: Run 'neptune deploy' to deploy your application.
        //         "#
        //         },
        //         name, final_image_uri
        //     );
        // } else {
        //     println!("✅ Build successful: {}", final_image_uri);
        //     println!("Next: neptune deploy");
        // }
        Ok(Some(final_image_uri))
    }
}
