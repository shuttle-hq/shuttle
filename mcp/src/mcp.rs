use rmcp::{
    model::{ServerCapabilities, ServerInfo},
    tool, ServerHandler,
};

use crate::utils::execute_command;

#[derive(Clone)]
pub struct ShuttleMcpServer;

#[tool(tool_box)]
impl ShuttleMcpServer {
    #[tool(description = "Deploy a project")]
    async fn deploy(
        &self,
        #[tool(param)]
        #[schemars(description = "The current working directory")]
        cwd: String,
        #[tool(param)]
        #[schemars(description = "Allow deployment with uncommitted files")]
        allow_dirty: bool,
        #[tool(param)]
        #[schemars(
            description = "Output the deployment archive to a file instead of sending a deployment request"
        )]
        output_archive: Option<String>,
        #[tool(param)]
        #[schemars(
            description = "Don't follow the deployment status, exit after the operation begins"
        )]
        no_follow: bool,
        #[tool(param)]
        #[schemars(description = "Don't display timestamps and log origin tags")]
        raw: bool,
        #[tool(param)]
        #[schemars(description = "Use this secrets file instead")]
        secrets: Option<String>,
        #[tool(param)]
        #[schemars(description = "The name of the project to deploy")]
        name: Option<String>,
    ) -> Result<String, String> {
        let mut args = vec!["deploy".to_string()];

        if allow_dirty {
            args.push("--allow-dirty".to_string());
        }

        if let Some(output_archive) = output_archive {
            args.push("--output-archive".to_string());
            args.push(output_archive);
        }

        if let Some(name) = name {
            args.push("--name".to_string());
            args.push(name);
        }

        if no_follow {
            args.push("--no-follow".to_string());
        }

        if raw {
            args.push("--raw".to_string());
        }

        if let Some(secrets) = secrets {
            args.push("--secrets".to_string());
            args.push(secrets);
        }

        execute_command("shuttle", args, &cwd).await
    }
}

#[tool(tool_box)]
impl ServerHandler for ShuttleMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "A Shuttle API MCP server that provides deployment functionality".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
