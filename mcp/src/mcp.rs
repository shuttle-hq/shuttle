use crate::tools::{
    account::*, certificate::*, deployment::*, docs::*, feedback::*, generate::*, logs::*,
    project::*, resource::*, upgrade::*,
};

use crate::utils::run_tool;
use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

#[derive(Clone)]
pub struct ShuttleMcpServer;

#[tool(tool_box)]
impl ShuttleMcpServer {
    #[tool(description = "Deploy a project")]
    async fn deploy(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "WIP: Deploy this Docker image instead of building one")]
        image: Option<String>,
        #[tool(param)]
        #[schemars(description = "Allow deployment with uncommitted files")]
        allow_dirty: Option<bool>,
        #[tool(param)]
        #[schemars(
            description = "Output the deployment archive to a file instead of sending a deployment request"
        )]
        output_archive: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Use this secrets file instead")]
        secrets: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            deploy(
                cwd,
                DeployParams {
                    image,
                    allow_dirty,
                    output_archive,
                    no_follow,
                    raw,
                    secrets,
                    offline,
                    debug,
                    name,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "List the deployments for a service")]
    async fn deployment_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Which page to display")]
        page: Option<u32>,
        #[tool(param)]
        #[schemars(description = "How many deployments per page to display")]
        limit: Option<u32>,
        #[tool(param)]
        #[schemars(description = "Output tables without borders")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            deployment_list(
                cwd,
                DeploymentListParams {
                    page,
                    limit,
                    raw,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "View status of a deployment")]
    async fn deployment_status(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "ID of deployment to get status for")]
        id: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            deployment_status(
                cwd,
                DeploymentStatusParams {
                    id,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Redeploy a previous deployment")]
    async fn deployment_redeploy(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "ID of deployment to redeploy")]
        id: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            deployment_redeploy(
                cwd,
                DeploymentRedeployParams {
                    id,
                    no_follow,
                    raw,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Stop running deployment(s)")]
    async fn deployment_stop(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            deployment_stop(
                cwd,
                DeploymentStopParams {
                    no_follow,
                    raw,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "View build and deployment logs")]
    async fn logs(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(
            description = "Deployment ID to get logs for. Defaults to the current deployment"
        )]
        id: Option<String>,
        #[tool(param)]
        #[schemars(description = "View logs from the most recent deployment")]
        latest: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Follow log output")]
        follow: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "View the first N log lines")]
        head: Option<u32>,
        #[tool(param)]
        #[schemars(description = "View the last N log lines")]
        tail: Option<u32>,
        #[tool(param)]
        #[schemars(description = "View all log lines")]
        all: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Get logs from all deployments instead of one deployment")]
        all_deployments: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            logs(
                cwd,
                LogsParams {
                    id,
                    latest,
                    follow,
                    raw,
                    head,
                    tail,
                    all,
                    all_deployments,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Create a project on Shuttle")]
    async fn project_create(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            project_create(
                cwd,
                ProjectCreateParams {
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Update project config - rename the project")]
    async fn project_update_name(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "New name for the project")]
        new_name: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            project_update_name(
                cwd,
                ProjectUpdateNameParams {
                    new_name,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Get the status of this project on Shuttle")]
    async fn project_status(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            project_status(
                cwd,
                ProjectStatusParams {
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "List all projects you have access to")]
    async fn project_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Output tables without borders")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            project_list(
                cwd,
                ProjectListParams {
                    raw,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Delete a project and all linked data")]
    async fn project_delete(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Skip confirmations and proceed")]
        yes: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            project_delete(
                cwd,
                ProjectDeleteParams {
                    yes,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Link this workspace to a Shuttle project")]
    async fn project_link(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            project_link(
                cwd,
                ProjectLinkParams {
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "List project resources")]
    async fn resource_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Show secrets from resources")]
        show_secrets: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Output tables without borders")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            resource_list(
                cwd,
                ResourceListParams {
                    show_secrets,
                    raw,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Delete a resource")]
    async fn resource_delete(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Type of the resource to delete")]
        resource_type: String,
        #[tool(param)]
        #[schemars(description = "Skip confirmations and proceed")]
        yes: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            resource_delete(
                cwd,
                ResourceDeleteParams {
                    resource_type,
                    yes,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Add SSL certificate for custom domain")]
    async fn certificate_add(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Domain name")]
        domain: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            certificate_add(
                cwd,
                CertificateAddParams {
                    domain,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "List project certificates")]
    async fn certificate_list(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Output tables without borders")]
        raw: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            certificate_list(
                cwd,
                CertificateListParams {
                    raw,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Delete SSL certificate")]
    async fn certificate_delete(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Domain name")]
        domain: String,
        #[tool(param)]
        #[schemars(description = "Skip confirmations and proceed")]
        yes: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            certificate_delete(
                cwd,
                CertificateDeleteParams {
                    domain,
                    yes,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Show account info")]
    async fn account(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            account(
                cwd,
                AccountParams {
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Generate shell completions")]
    async fn generate_shell(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "The shell to generate completion for")]
        shell: String,
        #[tool(param)]
        #[schemars(description = "Output to a file (stdout by default)")]
        output_file: Option<String>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            generate_shell(
                cwd,
                GenerateShellParams {
                    shell,
                    output_file,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Generate man page")]
    async fn generate_manpage(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            generate_manpage(
                cwd,
                GenerateManpageParams {
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Open GitHub issue for feedback")]
    async fn feedback(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            feedback(
                cwd,
                FeedbackParams {
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Upgrade CLI binary")]
    async fn upgrade(
        &self,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Install unreleased version from main branch")]
        preview: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the name or id of the project")]
        name: Option<String>,
        #[tool(param)]
        #[schemars(description = "Disable network requests that are not strictly necessary")]
        offline: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Turn on tracing output for Shuttle libraries")]
        debug: Option<bool>,
        #[tool(param)]
        #[schemars(description = "Specify the working directory")]
        working_directory: Option<String>,
    ) -> Result<String, String> {
        run_tool(|| async {
            upgrade(
                cwd,
                UpgradeParams {
                    preview,
                    name,
                    offline,
                    debug,
                    working_directory,
                },
            )
            .await
        })
        .await
    }

    #[tool(description = "Search Shuttle documentation")]
    async fn search_docs(
        &self,
        #[tool(param)]
        #[schemars(description = "Search query for documentation")]
        query: String,
    ) -> Result<String, String> {
        run_tool(|| async { search_docs(query).await }).await
    }
}

#[tool(tool_box)]
impl ServerHandler for ShuttleMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Shuttle MCP server providing CLI deployment and project management tools".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
