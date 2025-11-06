use std::collections::hash_map::DefaultHasher;
use std::env;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use bollard::{auth::DockerCredentials, Docker};
use futures::TryStreamExt;
use ignore::{gitignore::GitignoreBuilder, Match};
use nixpacks::nixpacks::{
    builder::docker::DockerBuilderOptions,
    plan::{generator::GeneratePlanOptions, BuildPlan},
};
use tar::{Builder, Header};
use walkdir::WalkDir;

use crate::impulse::{args::DeployArgs, Impulse};

impl Impulse {
    fn load_dockerignore(
        &self,
        dockerfile_path: &Path,
    ) -> Result<Option<ignore::gitignore::Gitignore>> {
        let dockerignore_path = dockerfile_path.join(".dockerignore");
        if !dockerignore_path.exists() {
            return Ok(None);
        }

        let mut builder = GitignoreBuilder::new(dockerfile_path);
        builder.add(&dockerignore_path);
        Ok(Some(builder.build()?))
    }

    fn create_build_context(
        &self,
        dockerfile_context_root: &Path,
        dockerfile_filename: &Path,
    ) -> Result<Vec<u8>> {
        let mut tar_data = Vec::new();
        {
            let mut tar = Builder::new(&mut tar_data);
            let ignore_spec = self.load_dockerignore(dockerfile_context_root)?;

            tracing::debug!(
                "Creating build context tar from {:?}",
                dockerfile_context_root
            );

            for entry in WalkDir::new(dockerfile_context_root).into_iter() {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    let rel_path = path.strip_prefix(dockerfile_context_root)?;

                    // Check against .dockerignore patterns
                    if let Some(ref ignore) = ignore_spec {
                        if let Match::Ignore(_) = ignore.matched(rel_path, false) {
                            tracing::debug!("Ignoring file due to .dockerignore: {:?}", rel_path);
                            continue;
                        }
                    }
                    tracing::debug!("Adding tar entry: {:?}", rel_path);

                    tar.append_path_with_name(path, rel_path)?;
                }
            }

            tracing::debug!("{:?}", dockerfile_context_root);

            // Add the Dockerfile to the tar
            let dockerfile_path = dockerfile_context_root.join(dockerfile_filename);
            if dockerfile_path.exists() && dockerfile_path.is_file() {
                let mut dockerfile_contents = Vec::new();
                File::open(&dockerfile_path)?.read_to_end(&mut dockerfile_contents)?;

                let mut header = Header::new_gnu();
                header.set_path("Dockerfile")?;
                header.set_size(dockerfile_contents.len() as u64);
                header.set_cksum();

                tar.append(&header, dockerfile_contents.as_slice())?;
            }

            tar.finish()?;
        }

        Ok(tar_data)
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

        if (build_args.dockerfile.is_some()
            && tokio::fs::try_exists(build_args.dockerfile.as_ref().unwrap()).await?)
            || tokio::fs::try_exists("Dockerfile").await?
        {
            println!("Using local Dockerfile...");
            let tar: Vec<u8> = self.create_build_context(
                wd,
                &build_args
                    .dockerfile
                    .unwrap_or_else(|| Path::new("Dockerfile").to_path_buf()),
            )?;
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

            nixpacks::create_docker_image(
                rel_path,
                build_args.env.iter().map(|e| e.as_str()).collect(),
                &GeneratePlanOptions {
                    plan: Some(BuildPlan::default()),
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
        //             "next_action_tool": "impulse-deploy",
        //             "next_action_params": {{
        //                 "image_uri": "{}"
        //             }},
        //             "next_action_non_tool": "Run 'impulse deploy' to deploy the built image to your infrastructure."
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

        //         Next step: Run 'impulse deploy' to deploy your application.
        //         "#
        //         },
        //         name, final_image_uri
        //     );
        // } else {
        //     println!("✅ Build successful: {}", final_image_uri);
        //     println!("Next: impulse deploy");
        // }
        Ok(Some(final_image_uri))
    }
}
