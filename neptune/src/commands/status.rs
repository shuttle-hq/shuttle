use anyhow::Result;
use cargo_shuttle::args::OutputMode;

use crate::{Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn status(&self) -> Result<NeptuneCommandOutput> {
        let spec = match self.fetch_local_state().await {
            Ok(project) => project,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                if self.global_args.output_mode == OutputMode::Json {
                    eprintln!(indoc::indoc! {r#"
                        {{
                            "error": "shuttle_json_not_found",
                            "message": "The shuttle.json project manifest file was not found in the current directory",
                            "suggestion": "Run 'neptune generate' to create the shuttle.json configuration file",
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
                        1. Run 'neptune generate' to create a new shuttle.json configuration
                        2. Configure your project settings in the generated file
                        3. Run 'neptune status' again to check deployment status
                        "#
                    });
                } else {
                    eprintln!("ERROR: shuttle.json not found - run 'neptune generate' to create the project manifest");
                }
                return Ok(NeptuneCommandOutput::None);
            }
            _ => return Ok(NeptuneCommandOutput::None),
        };

        let projects = self.client.get_projects().await?.into_inner();

        if let Some(status) = projects
            .into_iter()
            .find(|x| x.name == spec.name)
            .map(|p| p.condition)
        {
            Ok(NeptuneCommandOutput::ProjectStatus(Box::new(status)))
        } else {
            if self.global_args.output_mode == OutputMode::Json {
                eprintln!(
                    indoc::indoc! {r#"
                    {{
                        "error": "project_not_deployed",
                        "message": "The project '{}' exists locally but was not found in the remote Shuttle platform",
                        "suggestion": "Run 'neptune deploy' to build and deploy this project to Shuttle",
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
                    1. Run 'neptune deploy' to build your application and deploy it to Shuttle
                    2. Wait for the deployment to complete (this may take several minutes)
                    3. Run 'neptune status' again to check the deployment status
                    
                    Note: The first deployment may take longer as it needs to build and
                    provision all required resources.
                    "#
                    },
                    spec.name
                );
            } else {
                eprintln!(
                    "ERROR: Project '{}' not deployed - run 'neptune deploy' to deploy the project",
                    spec.name
                );
            }
            Ok(NeptuneCommandOutput::None)
        }
    }
}
