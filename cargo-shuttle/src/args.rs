use std::{
    ffi::OsString,
    fs::create_dir_all,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{
    builder::{OsStringValueParser, PossibleValue, TypedValueParser},
    Args, Parser, Subcommand, ValueEnum,
};
use clap_complete::Shell;
use serde::{Deserialize, Serialize};
use shuttle_common::{
    constants::EXAMPLES_REPO,
    models::{deployment::BuildMeta, resource::ResourceType},
};

use crate::util::cargo_metadata;

#[derive(Parser)]
#[command(
    version,
    next_help_heading = "Global options",
    // When running 'cargo shuttle', Cargo passes in the subcommand name to the invoked executable.
    // Use a hidden, optional positional argument to deal with it.
    arg(clap::Arg::new("dummy")
        .value_parser([PossibleValue::new("shuttle")])
        .required(false)
        .hide(true))
)]
pub struct ShuttleArgs {
    /// Target a different Shuttle API env (use a separate global config) (default: None (= prod = production))
    // ("SHUTTLE_ENV" is used for user-facing environments (agnostic of Shuttle API env))
    #[arg(global = true, long, env = "SHUTTLE_API_ENV", hide = true)]
    pub api_env: Option<String>,
    /// URL for the Shuttle API to target (overrides inferred URL from api_env)
    #[arg(global = true, long, env = "SHUTTLE_API", hide = true)]
    pub api_url: Option<String>,
    /// Modify Shuttle API URL to use admin endpoints
    #[arg(global = true, long, env = "SHUTTLE_ADMIN", hide = true)]
    pub admin: bool,
    /// Disable network requests that are not strictly necessary. Limits some features.
    #[arg(global = true, long, env = "SHUTTLE_OFFLINE")]
    pub offline: bool,
    /// Turn on tracing output for Shuttle libraries. (WARNING: can print sensitive data)
    #[arg(global = true, long, env = "SHUTTLE_DEBUG")]
    pub debug: bool,
    /// What format to print output in
    #[arg(
        global = true,
        long = "output",
        env = "SHUTTLE_OUTPUT_MODE",
        default_value = "normal"
    )]
    pub output_mode: OutputMode,
    #[command(flatten)]
    pub project_args: ProjectArgs,

    #[command(subcommand)]
    pub cmd: Command,
}

#[derive(ValueEnum, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    #[default]
    Normal,
    Json,
    // TODO?: add table / non-table / raw table / raw logs variants?
}

/// Global project-related options
#[derive(Args, Clone, Debug)]
pub struct ProjectArgs {
    #[arg(global = true, long, visible_alias = "wd", default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_path))]
    pub working_directory: PathBuf,
    /// The name of the project to target or create
    #[arg(global = true, long)]
    pub name: Option<String>,
    /// The id of the project to target
    #[arg(global = true, long)]
    pub id: Option<String>,
}

impl ProjectArgs {
    pub fn working_directory(&self) -> &Path {
        self.working_directory.as_path()
    }

    pub fn workspace_path(&self) -> PathBuf {
        self.cargo_workspace_path()
            .unwrap_or(self.working_directory.clone())
    }

    fn cargo_workspace_path(&self) -> anyhow::Result<PathBuf> {
        cargo_metadata(self.working_directory.as_path()).map(|meta| meta.workspace_root.into())
    }

    /// Try to use the workspace root package name if it exists,
    ///     else the name of the cargo workspace root dir,
    ///     else the name of the working dir.
    /// Errors on invalid dir names.
    pub fn local_project_name(&self) -> anyhow::Result<String> {
        Ok(
            if let Some(name) = cargo_metadata(self.working_directory.as_path())
                .ok()
                .and_then(|meta| meta.root_package().map(|rp| rp.to_owned()))
                .map(|rp| rp.name.to_string())
            {
                name
            } else {
                self.workspace_path()
                    .file_name()
                    .context("expected workspace path to have name")?
                    .to_os_string()
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("workspace path name is not valid unicode"))?
            },
        )
    }
}

#[allow(rustdoc::bare_urls)]
/// CLI for the Shuttle platform (https://www.shuttle.dev/)
///
/// See the CLI docs for more information: https://docs.shuttle.dev/guides/cli
#[derive(Subcommand)]
pub enum Command {
    /// Generate a Shuttle project from a template
    #[command(visible_alias = "i")]
    Init(InitArgs),
    /// Run a project locally
    #[command(visible_alias = "r")]
    Run(RunArgs),
    /// Build a project
    #[command(visible_alias = "b", hide = true)]
    Build(BuildArgs),
    /// Deploy a project
    #[command(visible_alias = "d")]
    Deploy(DeployArgs),
    /// Manage deployments
    #[command(subcommand, visible_alias = "depl")]
    Deployment(DeploymentCommand),
    /// View build and deployment logs
    Logs(LogsArgs),
    /// Manage Shuttle projects
    #[command(subcommand, visible_alias = "proj")]
    Project(ProjectCommand),
    /// Manage resources
    #[command(subcommand, visible_alias = "res")]
    Resource(ResourceCommand),
    /// Manage SSL certificates for custom domains
    #[command(subcommand, visible_alias = "cert")]
    Certificate(CertificateCommand),
    /// Show info about your Shuttle account
    #[command(visible_alias = "acc")]
    Account,
    /// Log in to the Shuttle platform
    Login(LoginArgs),
    /// Log out of the Shuttle platform
    Logout(LogoutArgs),
    /// Generate shell completions and man page
    #[command(subcommand)]
    Generate(GenerateCommand),
    /// Open an issue on GitHub and provide feedback
    Feedback,
    /// Upgrade the Shuttle CLI binary
    Upgrade {
        /// Install an unreleased version from the repository's main branch
        #[arg(long)]
        preview: bool,
    },
    /// Commands for the Shuttle MCP server
    #[command(subcommand)]
    Mcp(McpCommand),
}

#[derive(Subcommand)]
pub enum McpCommand {
    /// Start the Shuttle MCP server
    Start,
}

#[derive(Subcommand)]
pub enum GenerateCommand {
    /// Generate shell completions
    Shell {
        /// The shell to generate shell completion for
        shell: Shell,
        /// Output to a file (stdout by default)
        #[arg(short, long)]
        output_file: Option<PathBuf>,
    },
    /// Generate man page to the standard output
    Manpage,
}

#[derive(Args)]
#[command(next_help_heading = "Table options")]
pub struct TableArgs {
    /// Output tables without borders
    #[arg(long, default_value_t = false)]
    pub raw: bool,
}

#[derive(Subcommand)]
pub enum DeploymentCommand {
    /// List the deployments for a service
    #[command(visible_alias = "ls")]
    List {
        /// Which page to display
        #[arg(long, default_value = "1")]
        page: u32,

        /// How many deployments per page to display
        #[arg(long, default_value = "10", visible_alias = "per-page")]
        limit: u32,

        #[command(flatten)]
        table: TableArgs,
    },
    /// View status of a deployment
    #[command(visible_alias = "stat")]
    Status {
        /// ID of deployment to get status for
        deployment_id: Option<String>,
    },
    /// Redeploy a previous deployment (if possible)
    Redeploy {
        /// ID of deployment to redeploy
        deployment_id: Option<String>,

        #[command(flatten)]
        tracking_args: DeploymentTrackingArgs,
    },
    /// Stop running deployment(s)
    Stop {
        #[command(flatten)]
        tracking_args: DeploymentTrackingArgs,
    },
}

#[derive(Subcommand)]
pub enum ResourceCommand {
    /// List the resources for a project
    #[command(visible_alias = "ls")]
    List {
        /// Show secrets from resources (e.g. a password in a connection string)
        #[arg(long, default_value_t = false)]
        show_secrets: bool,

        #[command(flatten)]
        table: TableArgs,
    },
    /// Delete a resource
    #[command(visible_alias = "rm")]
    Delete {
        /// Type of the resource to delete.
        /// Use the string in the 'Type' column as displayed in the `resource list` command.
        /// For example, 'database::shared::postgres'.
        resource_type: ResourceType,
        #[command(flatten)]
        confirmation: ConfirmationArgs,
    },
    /// Dump a resource
    #[command(hide = true)] // not yet supported on shuttle.dev
    Dump {
        /// Type of the resource to dump.
        /// Use the string in the 'Type' column as displayed in the `resource list` command.
        /// For example, 'database::shared::postgres'.
        resource_type: ResourceType,
    },
}

#[derive(Subcommand)]
pub enum CertificateCommand {
    /// Add an SSL certificate for a custom domain
    Add {
        /// Domain name
        domain: String,
    },
    /// List the certificates for a project
    #[command(visible_alias = "ls")]
    List {
        #[command(flatten)]
        table: TableArgs,
    },
    /// Delete an SSL certificate
    #[command(visible_alias = "rm")]
    Delete {
        /// Domain name
        domain: String,
        #[command(flatten)]
        confirmation: ConfirmationArgs,
    },
}

#[derive(Subcommand)]
pub enum ProjectCommand {
    /// Create a project on Shuttle
    #[command(visible_alias = "start")]
    Create,
    /// Update project config
    #[command(subcommand, visible_alias = "upd")]
    Update(ProjectUpdateCommand),
    /// Get the status of this project on Shuttle
    #[command(visible_alias = "stat")]
    Status,
    /// List all projects you have access to
    #[command(visible_alias = "ls")]
    List {
        #[command(flatten)]
        table: TableArgs,
    },
    /// Delete a project and all linked data
    #[command(visible_alias = "rm")]
    Delete(ConfirmationArgs),
    /// Link this workspace to a Shuttle project
    Link,
}

#[derive(Subcommand, Debug)]
pub enum ProjectUpdateCommand {
    /// Rename the project, including its default subdomain
    Name { new_name: String },
}

#[derive(Args, Debug)]
pub struct ConfirmationArgs {
    /// Skip confirmations and proceed
    #[arg(long, short, default_value_t = false)]
    pub yes: bool,
}

#[derive(Args, Clone, Debug, Default)]
#[command(next_help_heading = "Login options")]
pub struct LoginArgs {
    /// Prompt to paste the API key instead of opening the browser
    #[arg(long, conflicts_with = "api_key", alias = "input")]
    pub prompt: bool,
    /// Log in with this Shuttle API key
    #[arg(long)]
    pub api_key: Option<String>,
    /// URL to the Shuttle Console for automatic login
    #[arg(long, env = "SHUTTLE_CONSOLE", hide = true)]
    pub console_url: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct LogoutArgs {
    /// Reset the API key before logging out
    #[arg(long)]
    pub reset_api_key: bool,
}

#[derive(Args, Default)]
pub struct DeployArgs {
    /// WIP: Deploy this Docker image instead of building one
    #[arg(long, short = 'i', hide = true)]
    pub image: Option<String>,

    /// Allow deployment with uncommitted files
    #[arg(long, visible_alias = "ad")]
    pub allow_dirty: bool,
    /// Output the deployment archive to a file instead of sending a deployment request
    #[arg(long)]
    pub output_archive: Option<PathBuf>,

    #[command(flatten)]
    pub tracking_args: DeploymentTrackingArgs,

    #[command(flatten)]
    pub secret_args: SecretsArgs,

    /// Use these build meta fields instead of discovering them
    #[arg(skip)]
    pub _build_meta: Option<BuildMeta>,
}
#[derive(Args, Default)]
pub struct DeploymentTrackingArgs {
    /// Don't follow the deployment status, exit after the operation begins
    #[arg(long, visible_alias = "nf")]
    pub no_follow: bool,
    /// Don't display timestamps and log origin tags
    #[arg(long)]
    pub raw: bool,
}

#[derive(Args, Debug, Default)]
pub struct RunArgs {
    /// Port to start service on
    #[arg(long, short = 'p', env, default_value = "8000")]
    pub port: u16,
    /// Use 0.0.0.0 instead of localhost (for usage with local external devices)
    #[arg(long)]
    pub external: bool,
    /// Don't display timestamps and log origin tags
    #[arg(long)]
    pub raw: bool,

    #[command(flatten)]
    pub secret_args: SecretsArgs,
    #[command(flatten)]
    pub build_args: BuildArgsShared,
}

#[derive(Args, Debug, Default)]
pub struct BuildArgs {
    /// Output the build archive to a file instead of building
    #[arg(long)]
    pub output_archive: Option<PathBuf>,
    #[command(flatten)]
    pub inner: BuildArgsShared,
}

/// Arguments shared by build and run commands
#[derive(Args, Debug, Default)]
pub struct BuildArgsShared {
    /// Use release mode for building the project
    #[arg(long, short = 'r')]
    pub release: bool,
    /// Uses bacon crate to build/run the project in watch mode
    #[arg(long)]
    pub bacon: bool,

    // Docker-related args
    /// Build/Run with docker instead of natively
    #[arg(long, hide = true)]
    pub docker: bool,
    /// Additional tag for the docker image
    #[arg(long, short = 't', requires = "docker", hide = true)]
    pub tag: Option<String>,
}

#[derive(Args, Debug, Default)]
pub struct SecretsArgs {
    /// Use this secrets file instead
    #[arg(long, value_parser = OsStringValueParser::new().try_map(parse_path))]
    pub secrets: Option<PathBuf>,
}

#[derive(Args, Clone, Debug, Default)]
pub struct InitArgs {
    /// Clone a starter template from Shuttle's official examples
    #[arg(long, short, value_enum, conflicts_with_all = &["from", "subfolder"])]
    pub template: Option<InitTemplateArg>,
    /// Clone a template from a git repository or local path
    #[arg(long)]
    pub from: Option<String>,
    /// Path to the template in the source (used with --from)
    #[arg(long, requires = "from")]
    pub subfolder: Option<String>,

    /// Path where to place the new Shuttle project
    #[arg(default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_and_create_path))]
    pub path: PathBuf,

    /// Don't check the project name's validity or availability and use it anyways
    #[arg(long)]
    pub force_name: bool,
    /// Whether to create a project on Shuttle
    #[arg(long, visible_alias = "create_env")]
    pub create_project: bool,
    /// Don't initialize a new git repository
    #[arg(long)]
    pub no_git: bool,

    #[command(flatten)]
    pub login_args: LoginArgs,
}

#[derive(ValueEnum, Clone, Debug, strum::EnumMessage, strum::VariantArray)]
pub enum InitTemplateArg {
    /// Axum - Modular web framework from the Tokio ecosystem
    Axum,
    /// Actix Web - Powerful and fast web framework
    ActixWeb,
    /// Rocket - Simple and easy-to-use web framework
    Rocket,
    /// Loco - Batteries included web framework based on Axum
    Loco,
    /// Salvo - Powerful and simple web framework
    Salvo,
    /// Poem - Full-featured and easy-to-use web framework
    Poem,
    /// Poise - Discord Bot framework with good slash command support
    Poise,
    /// Rama - Modular service framework to build proxies, servers and clients
    Rama,
    /// Serenity - Discord Bot framework
    Serenity,
    /// Tower - Modular service library
    Tower,
    /// Warp - Web framework
    Warp,
    /// No template - Make a custom service
    None,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateLocation {
    pub auto_path: String,
    pub subfolder: Option<String>,
}

impl InitArgs {
    /// Turns the template arg or git args to a repo+folder, if present.
    pub fn git_template(&self) -> anyhow::Result<Option<TemplateLocation>> {
        if self.from.is_some() && self.template.is_some() {
            anyhow::bail!("Template and From args can not be set at the same time.");
        }
        Ok(if let Some(from) = self.from.clone() {
            Some(TemplateLocation {
                auto_path: from,
                subfolder: self.subfolder.clone(),
            })
        } else {
            self.template.as_ref().map(|t| t.template())
        })
    }
}

impl InitTemplateArg {
    pub fn template(&self) -> TemplateLocation {
        use InitTemplateArg::*;
        let path = match self {
            ActixWeb => "actix-web/hello-world",
            Axum => "axum/hello-world",
            Loco => "loco/hello-world",
            Poem => "poem/hello-world",
            Poise => "poise/hello-world",
            Rocket => "rocket/hello-world",
            Salvo => "salvo/hello-world",
            Rama => "rama/hello-world",
            Serenity => "serenity/hello-world",
            Tower => "tower/hello-world",
            Warp => "warp/hello-world",
            None => "custom-service/none",
        };

        TemplateLocation {
            auto_path: EXAMPLES_REPO.into(),
            subfolder: Some(path.to_string()),
        }
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct LogsArgs {
    /// Deployment ID to get logs for. Defaults to the current deployment
    pub deployment_id: Option<String>,
    #[arg(short, long)]
    /// View logs from the most recent deployment (which is not always the running one)
    pub latest: bool,
    #[arg(short, long, hide = true)]
    /// Follow log output
    pub follow: bool,
    /// Don't display timestamps and log origin tags
    #[arg(long)]
    pub raw: bool,
    /// View the first N log lines
    #[arg(long, group = "pagination", hide = true)]
    pub head: Option<u32>,
    /// View the last N log lines
    #[arg(long, group = "pagination", hide = true)]
    pub tail: Option<u32>,
    /// View all log lines
    #[arg(long, group = "pagination", hide = true)]
    pub all: bool,
    /// Get logs from all deployments instead of one deployment
    #[arg(long, hide = true)]
    pub all_deployments: bool,
}

/// Helper function to parse and return the absolute path
pub fn parse_path(path: OsString) -> Result<PathBuf, io::Error> {
    dunce::canonicalize(&path).map_err(|e| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("could not turn {path:?} into a real path: {e}"),
        )
    })
}

/// Helper function to parse, create if not exists, and return the absolute path
pub(crate) fn parse_and_create_path(path: OsString) -> Result<PathBuf, io::Error> {
    // Create the directory if does not exist
    create_dir_all(&path).map_err(|e| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("Could not create directory: {e}"),
        )
    })?;

    parse_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::path_from_workspace_root;
    use clap::CommandFactory;

    #[test]
    fn test_shuttle_args() {
        ShuttleArgs::command().debug_assert();
    }

    #[test]
    fn test_init_args_framework() {
        // pre-defined template (only hello world)
        let init_args = InitArgs {
            template: Some(InitTemplateArg::Tower),
            from: None,
            subfolder: None,
            ..Default::default()
        };
        assert_eq!(
            init_args.git_template().unwrap(),
            Some(TemplateLocation {
                auto_path: EXAMPLES_REPO.into(),
                subfolder: Some("tower/hello-world".into())
            })
        );

        // pre-defined template (multiple)
        let init_args = InitArgs {
            template: Some(InitTemplateArg::Axum),
            from: None,
            subfolder: None,
            ..Default::default()
        };
        assert_eq!(
            init_args.git_template().unwrap(),
            Some(TemplateLocation {
                auto_path: EXAMPLES_REPO.into(),
                subfolder: Some("axum/hello-world".into())
            })
        );

        // pre-defined "none" template
        let init_args = InitArgs {
            template: Some(InitTemplateArg::None),
            from: None,
            subfolder: None,
            ..Default::default()
        };
        assert_eq!(
            init_args.git_template().unwrap(),
            Some(TemplateLocation {
                auto_path: EXAMPLES_REPO.into(),
                subfolder: Some("custom-service/none".into())
            })
        );

        // git template with path
        let init_args = InitArgs {
            template: None,
            from: Some("https://github.com/some/repo".into()),
            subfolder: Some("some/path".into()),
            ..Default::default()
        };
        assert_eq!(
            init_args.git_template().unwrap(),
            Some(TemplateLocation {
                auto_path: "https://github.com/some/repo".into(),
                subfolder: Some("some/path".into())
            })
        );

        // No template or repo chosen
        let init_args = InitArgs {
            template: None,
            from: None,
            subfolder: None,
            ..Default::default()
        };
        assert_eq!(init_args.git_template().unwrap(), None);
    }

    #[test]
    fn workspace_path() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
            id: None,
        };

        assert_eq!(
            project_args.workspace_path(),
            path_from_workspace_root("examples/axum/hello-world/")
        );
    }

    #[test]
    fn project_name() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
            id: None,
        };

        assert_eq!(project_args.local_project_name().unwrap(), "hello-world");
    }

    #[test]
    fn project_name_in_workspace() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root(
                "examples/rocket/workspace/hello-world/src",
            ),
            name: None,
            id: None,
        };

        assert_eq!(project_args.local_project_name().unwrap(), "workspace");
    }

    #[test]
    fn project_name_in_non_rust_dir() {
        let project_args = ProjectArgs {
            working_directory: "/home".into(),
            name: None,
            id: None,
        };

        assert_eq!(project_args.local_project_name().unwrap(), "home");
    }
}
