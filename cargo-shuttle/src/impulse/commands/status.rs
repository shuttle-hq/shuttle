use anyhow::Result;

use crate::impulse::{Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn status(&self) -> Result<ImpulseCommandOutput> {
        let spec = match self.fetch_local_state().await {
            Ok(project) => project,
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
                        for status operations. This file defines your project name, resources,
                        and deployment settings.
                        
                        To fix this issue:
                        1. Run 'impulse generate' to create a new shuttle.json configuration
                        2. Configure your project settings in the generated file
                        3. Run 'impulse status' again to check deployment status
                        "#
                    });
                } else {
                    eprintln!("ERROR: shuttle.json not found - run 'impulse generate' to create the project manifest");
                }
                return Ok(ImpulseCommandOutput::None);
            }
            _ => return Ok(ImpulseCommandOutput::None),
        };

        let projects = self.client.get_impulse_projects().await?.into_inner();

        if let Some(ref status) = projects
            .into_iter()
            .find(|x| x.name == spec.name)
            .and_then(|p| p.status)
        {
            Ok(ImpulseCommandOutput::ProjectStatus(Box::new(
                status.clone(),
            )))
        } else {
            if self.global_args.output_mode == crate::OutputMode::Json {
                eprintln!(
                    indoc::indoc! {r#"
                    {{
                        "error": "project_not_deployed",
                        "message": "The project '{}' exists locally but was not found in the remote Shuttle platform",
                        "suggestion": "Run 'impulse deploy' to build and deploy this project to Shuttle",
                        "next_action": "deploy_project",
                        "project_name": "{}"
                    }}"#
                    },
                    spec.name, spec.name
                );
            } else if self.global_args.verbose {
                eprintln!(
                    indoc::indoc! {r#"
                    ERROR: Project '{}' not deployed to Shuttle
                    
                    Your shuttle.json configuration was found locally, but the project
                    does not exist on the remote Shuttle platform. This means the project
                    has not been deployed yet.
                    
                    To fix this issue:
                    1. Run 'impulse deploy' to build your application and deploy it to Shuttle
                    2. Wait for the deployment to complete (this may take several minutes)
                    3. Run 'impulse status' again to check the deployment status
                    
                    Note: The first deployment may take longer as it needs to build and
                    provision all required resources.
                    "#
                    },
                    spec.name
                );
            } else {
                eprintln!(
                    "ERROR: Project '{}' not deployed - run 'impulse deploy' to deploy the project",
                    spec.name
                );
            }
            Ok(ImpulseCommandOutput::None)
        }
    }
}
