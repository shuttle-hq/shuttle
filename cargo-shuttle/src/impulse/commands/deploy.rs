use anyhow::Result;

use crate::impulse::{args::DeployArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn deploy(&self, deploy_args: DeployArgs) -> Result<ImpulseCommandOutput> {
        let spec = match self.fetch_local_state().await {
            Ok(project) => project,
            // Handle 'shuttle.json' file missing
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.global_args.output_mode == crate::OutputMode::Json {
                    eprintln!(indoc::indoc! {r#"
                        {{
                            "error": "shuttle_json_not_found",
                            "message": "The shuttle.json project manifest file was not found in the current directory",
                            "suggestion": "Run 'impulse generate' to create the shuttle.json configuration file",
                            "next_action": "generate_config"
                        }}"#
                    });
                } else if self.global_args.verbose {
                    eprintln!(indoc::indoc! {r#"
                        ERROR: shuttle.json project manifest not found
                        
                        The shuttle.json file contains your project configuration and is required
                        for build operations. This file defines your project name, resources,
                        and deployment settings.
                        
                        To fix this issue:
                        1. Run 'impulse generate' to create a new shuttle.json configuration
                        2. Configure your project settings in the generated file
                        3. Run 'impulse build' again to build your project
                        "#
                    });
                } else {
                    eprintln!("ERROR: shuttle.json not found - run 'impulse generate' to create the project manifest");
                }
                return Ok(ImpulseCommandOutput::None);
            }
            // Handle JSON validation errors (missing fields, etc.)
            Err(e) => {
                match e.downcast::<serde_json::Error>() {
                    Err(e) => {
                        eprintln!("ERROR: {:?}", e);
                    }
                    Ok(json_err) => {
                        // Handle other JSON errors
                        let err_message = format!("invalid shuttle.json - {}", json_err);
                        let err_message = err_message.replace('"', r#"\""#);
                        let json_err = json_err.to_string().replace('"', r#"\""#);

                        if self.global_args.output_mode == crate::OutputMode::Json {
                            eprintln!(
                                indoc::indoc! {r#"
                            {{
                                "ok": false,
                                "error": {{
                                    "code": "INVALID_CONFIG",
                                    "message": "{}",
                                    "details": {{
                                        "type": "json_parse_error",
                                        "raw_error": "{}"
                                    }}
                                }},
                                "next_action": "fix_config_then_retry",
                                "requires_confirmation": false,
                                "next_action_tool": "impulse-generate",
                                "next_action_params": {{
                                    "config": "<pass shuttle.json>",
                                    "request": "{}",
                                    "context": "<optional code snippets>"
                                }},
                                "next_action_non_tool": "Fix shuttle.json errors with impulse generate, then re-run validation."
                            }}"#, 
                                },
                                err_message, json_err, err_message
                            );
                        } else {
                            eprintln!(
                                "ERROR: {} - check shuttle.json syntax and structure",
                                err_message
                            );
                        }
                    }
                }
                return Ok(ImpulseCommandOutput::None);
            }
        };

        tracing::info!("Spec: {:?}", spec);

        if let Some(image_name) = self.build(&spec.name, deploy_args).await? {
            tracing::info!("Image name: {}", image_name);
            let project = if let Some(project_id) = self
                .client
                .get_impulse_project_id_from_name(&spec.name)
                .await?
            {
                self.client.get_impulse_project_by_id(&project_id).await?
            } else {
                self.client.create_impulse_project(&spec).await?
            }
            .into_inner();

            let deployment = self
                .client
                .create_impulse_deployment(&spec, &project.id, &image_name)
                .await?
                .into_inner();

            // Handle successful deployment output
            if self.global_args.output_mode == crate::OutputMode::Json {
                println!(
                    indoc::indoc! {r#"
                    {{
                        "ok": true,
                        "project": "{}",
                        "deployment_id": "{}",
                        "summary": "Changes applied successfully.",
                        "messages": ["Deployments may take a while."],
                        "next_action": "await_completion",
                        "requires_confirmation": false,
                        "next_action_tool": null,
                        "next_action_params": null,
                        "next_action_non_tool": "You can manually run impulse status later to view progress."
                    }}"#},
                    spec.name, deployment.id
                );
            } else {
                println!("✅ Deployment successful!");
                println!("📦 Project: {}", spec.name);
                println!("🚀 Deployment ID: {}", deployment.id);
                println!("⏳ Deployments may take a while to complete.");
                println!("💡 Run 'impulse status' to check deployment progress.");
            }
        } else {
            // Handle build failure
            if self.global_args.output_mode == crate::OutputMode::Json {
                eprintln!(
                    indoc::indoc! {r#"
                    {{
                        "ok": false,
                        "error": {{
                            "code": "BUILD_FAILED",
                            "message": "Build process failed to produce a container image",
                            "details": {{
                                "stage": "build",
                                "project_name": "{}"
                            }}
                        }},
                        "next_action": "check_build_logs_then_retry",
                        "requires_confirmation": false,
                        "next_action_tool": "impulse-build",
                        "next_action_params": {{
                            "config": "<pass shuttle.json>",
                            "verbose": true
                        }},
                        "next_action_non_tool": "Check build logs for errors, fix any issues, then retry the build process."
                    }}"#},
                    spec.name
                );
            } else {
                eprintln!("❌ Build failed - unable to create container image");
                eprintln!("📋 Project: {}", spec.name);
                eprintln!(
                    "💡 Check build logs for errors and retry with 'impulse build --verbose'"
                );
            }
            return Ok(ImpulseCommandOutput::None);
        }

        Ok(ImpulseCommandOutput::None)
    }
}
