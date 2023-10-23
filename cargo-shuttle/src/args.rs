use std::{
    ffi::OsString,
    fs::create_dir_all,
    io::{self, ErrorKind},
    path::PathBuf,
};

use anyhow::{bail, Context};
use cargo_metadata::MetadataCommand;
use clap::{
    builder::{OsStringValueParser, PossibleValue, TypedValueParser},
    Parser, ValueEnum,
};
use clap_complete::Shell;
use shuttle_common::{models::project::DEFAULT_IDLE_MINUTES, project::ProjectName, resource};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    version,
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(clap::Arg::new("dummy")
        .value_parser([PossibleValue::new("shuttle")])
        .required(false)
        .hide(true))
)]
pub struct ShuttleArgs {
    #[command(flatten)]
    pub project_args: ProjectArgs,
    /// Run this command against the API at the supplied URL
    /// (allows targeting a custom deployed instance for this command only, mainly for development)
    #[arg(long, env = "SHUTTLE_API")]
    pub api_url: Option<String>,
    #[command(subcommand)]
    pub cmd: Command,
}

// Common args for subcommands that deal with projects.
#[derive(Parser, Debug)]
pub struct ProjectArgs {
    /// Specify the working directory
    #[arg(global = true, long, alias = "wd", default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_init_path))]
    pub working_directory: PathBuf,
    /// Specify the name of the project (overrides crate name)
    #[arg(global = true, long)]
    pub name: Option<ProjectName>,
}

impl ProjectArgs {
    pub fn workspace_path(&self) -> anyhow::Result<PathBuf> {
        let path = MetadataCommand::new()
            .current_dir(&self.working_directory)
            .exec()
            .context("failed to get cargo metadata")?
            .workspace_root
            .into();

        Ok(path)
    }

    pub fn project_name(&self) -> anyhow::Result<ProjectName> {
        let workspace_path = self.workspace_path()?;

        let meta = MetadataCommand::new()
            .current_dir(&workspace_path)
            .exec()
            .unwrap();
        let package_name = if let Some(root_package) = meta.root_package() {
            root_package.name.clone().parse()?
        } else {
            workspace_path
                .file_name()
                .context("failed to get project name from workspace path")?
                .to_os_string()
                .into_string()
                .expect("workspace file name should be valid unicode")
                .parse()?
        };

        Ok(package_name)
    }
}

/// A cargo command for the Shuttle platform (https://www.shuttle.rs/)
///
/// See the CLI docs (https://docs.shuttle.rs/getting-started/shuttle-commands)
/// for more information.
#[derive(Parser)]
pub enum Command {
    /// Create a new Shuttle project
    Init(InitArgs),
    /// Run a Shuttle service locally
    Run(RunArgs),
    /// Deploy a Shuttle service
    Deploy(DeployArgs),
    /// Manage deployments of a Shuttle service
    #[command(subcommand)]
    Deployment(DeploymentCommand),
    /// View the status of a Shuttle service
    Status,
    /// Stop this Shuttle service
    Stop,
    /// View the logs of a deployment in this Shuttle service
    Logs {
        /// Deployment ID to get logs for. Defaults to currently running deployment
        id: Option<Uuid>,
        #[arg(short, long)]
        /// View logs from the most recent deployment (which is not always the latest running one)
        latest: bool,
        #[arg(short, long)]
        /// Follow log output
        follow: bool,
    },
    /// List or manage projects on Shuttle
    #[command(subcommand)]
    Project(ProjectCommand),
    /// Manage resources of a Shuttle project
    #[command(subcommand)]
    Resource(ResourceCommand),
    /// Manage secrets for this Shuttle service
    Secrets,
    /// Remove cargo build artifacts in the Shuttle environment
    Clean,
    /// Login to the Shuttle platform
    Login(LoginArgs),
    /// Log out of the Shuttle platform
    Logout(LogoutArgs),
    /// Generate shell completions
    Generate {
        /// Which shell
        #[arg(short, long, env, default_value_t = Shell::Bash)]
        shell: Shell,
        /// Output to a file (stdout by default)
        #[arg(short, long, env)]
        output: Option<PathBuf>,
    },
    /// Open an issue on GitHub and provide feedback
    Feedback,
}

#[derive(Parser)]
pub enum DeploymentCommand {
    /// List all the deployments for a service
    List {
        #[arg(long, default_value = "1")]
        /// Which page to display
        page: u32,

        #[arg(long, default_value = "10")]
        /// How many projects per page to display
        limit: u32,
    },
    /// View status of a deployment
    Status {
        /// ID of deployment to get status for
        id: Uuid,
    },
}

#[derive(Parser)]
pub enum ResourceCommand {
    /// List all the resources for a project
    List,
    /// Delete a resource
    Delete {
        /// Type of the resource to delete.
        /// Use the string in the 'Type' column as displayed in the `resource list` command.
        /// For example, 'database::shared::postgres'.
        resource_type: resource::Type,
    },
}

#[derive(Parser)]
pub enum ProjectCommand {
    /// Create an environment for this project on Shuttle
    Start(ProjectStartArgs),
    /// Check the status of this project's environment on Shuttle
    Status {
        #[arg(short, long)]
        /// Follow status of project command
        follow: bool,
    },
    /// Destroy this project's environment (container) on Shuttle
    Stop,
    /// Destroy and create an environment for this project on Shuttle
    Restart(ProjectStartArgs),
    /// List all projects belonging to the calling account
    List {
        #[arg(long, default_value = "1")]
        /// Which page to display
        page: u32,

        #[arg(long, default_value = "10")]
        /// How many projects per page to display
        limit: u32,
    },
    /// Delete project. This also deletes associated Secrets and Persist data.
    Delete,
}

#[derive(Parser, Debug)]
pub struct ProjectStartArgs {
    #[arg(long, default_value_t = DEFAULT_IDLE_MINUTES)]
    /// How long to wait before putting the project in an idle state due to inactivity.
    /// 0 means the project will never idle
    pub idle_minutes: u64,
}

#[derive(Parser, Clone, Debug)]
pub struct LoginArgs {
    /// API key for the Shuttle platform
    #[arg(long)]
    pub api_key: Option<String>,
}

#[derive(Parser, Clone, Debug)]
pub struct LogoutArgs {
    /// Reset the API key before logging out
    #[arg(long)]
    pub reset_api_key: bool,
}
#[derive(Parser)]
pub struct DeployArgs {
    /// Allow deployment with uncommited files
    #[arg(long, alias = "ad")]
    pub allow_dirty: bool,
    /// Don't run pre-deploy tests
    #[arg(long, alias = "nt")]
    pub no_test: bool,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Port to start service on
    #[arg(long, short = 'p', env, default_value = "8000")]
    pub port: u16,
    /// Use 0.0.0.0 instead of localhost (for usage with local external devices)
    #[arg(long)]
    pub external: bool,
    /// Use release mode for building the project
    #[arg(long, short = 'r')]
    pub release: bool,
}

#[derive(Parser, Clone, Debug)]
pub struct InitArgs {
    /// Clone a starter template from Shuttle's official examples
    #[arg(long, short, value_enum, conflicts_with_all = &["from", "subfolder"])]
    pub template: Option<InitTemplateArg>,
    /// Clone a template from a git repository or local path using cargo-generate
    #[arg(long)]
    pub from: Option<String>,
    /// Path to the template in the source (used with --from)
    #[arg(long, requires = "from")]
    pub subfolder: Option<String>,

    /// Path where to place the new Shuttle project
    #[arg(default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_init_path))]
    pub path: PathBuf,

    /// Whether to start the container for this project on Shuttle, and claim the project name
    #[arg(long)]
    pub create_env: bool,
    #[command(flatten)]
    pub login_args: LoginArgs,
}

#[derive(ValueEnum, Clone, Debug, strum::Display, strum::EnumIter)]
#[strum(serialize_all = "kebab-case")]
pub enum InitTemplateArg {
    /// Actix Web framework
    ActixWeb,
    /// Axum web framework
    Axum,
    /// Poem web framework
    Poem,
    /// Poise Discord framework
    Poise,
    /// Rocket web framework
    Rocket,
    /// Salvo web framework
    Salvo,
    /// Serenity Discord framework
    Serenity,
    /// Thruster web framework
    Thruster,
    /// Tide web framework
    Tide,
    /// Tower web framework
    Tower,
    /// Warp web framework
    Warp,
    /// No template - Custom empty service
    None,
}

pub const EXAMPLES_REPO: &str = "https://github.com/shuttle-hq/shuttle-examples";

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateLocation {
    pub auto_path: String,
    pub subfolder: Option<String>,
}

impl InitArgs {
    pub fn git_template(&self) -> anyhow::Result<Option<TemplateLocation>> {
        if self.from.is_some() && self.template.is_some() {
            bail!("Template and From args can not be set at the same time.");
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
            Poem => "poem/hello-world",
            Poise => "poise/hello-world",
            Rocket => "rocket/hello-world",
            Salvo => "salvo/hello-world",
            Serenity => "serenity/hello-world",
            Thruster => "thruster/hello-world",
            Tide => "tide/hello-world",
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

/// Helper function to parse and return the absolute path
fn parse_path(path: OsString) -> Result<PathBuf, String> {
    dunce::canonicalize(&path).map_err(|e| format!("could not turn {path:?} into a real path: {e}"))
}

/// Helper function to parse, create if not exists, and return the absolute path
pub(crate) fn parse_init_path(path: OsString) -> Result<PathBuf, io::Error> {
    // Create the directory if does not exist
    create_dir_all(&path).map_err(|e| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("Could not create directory: {e}"),
        )
    })?;

    parse_path(path).map_err(|e| io::Error::new(ErrorKind::InvalidInput, e))
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
            create_env: false,
            login_args: LoginArgs { api_key: None },
            path: PathBuf::new(),
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
            create_env: false,
            login_args: LoginArgs { api_key: None },
            path: PathBuf::new(),
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
            create_env: false,
            login_args: LoginArgs { api_key: None },
            path: PathBuf::new(),
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
            create_env: false,
            login_args: LoginArgs { api_key: None },
            path: PathBuf::new(),
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
            create_env: false,
            login_args: LoginArgs { api_key: None },
            path: PathBuf::new(),
        };
        assert_eq!(init_args.git_template().unwrap(), None);
    }

    #[test]
    fn workspace_path() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
        };

        assert_eq!(
            project_args.workspace_path().unwrap(),
            path_from_workspace_root("examples/axum/hello-world/")
        );
    }

    #[test]
    fn project_name() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
            name: None,
        };

        assert_eq!(
            project_args.project_name().unwrap().to_string(),
            "hello-world"
        );
    }

    #[test]
    fn project_name_in_workspace() {
        let project_args = ProjectArgs {
            working_directory: path_from_workspace_root(
                "examples/rocket/workspace/hello-world/src",
            ),
            name: None,
        };

        assert_eq!(
            project_args.project_name().unwrap().to_string(),
            "workspace"
        );
    }
}
