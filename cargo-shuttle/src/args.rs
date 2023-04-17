use std::{
    ffi::OsString,
    fs::create_dir_all,
    io::{self, ErrorKind},
    path::PathBuf,
};

use anyhow::Context;
use cargo_metadata::MetadataCommand;
use clap::builder::{OsStringValueParser, PossibleValue, TypedValueParser};
use clap::Parser;
use clap_complete::Shell;
use shuttle_common::{models::project::IDLE_MINUTES, project::ProjectName};
use uuid::Uuid;

use crate::init::Framework;

#[derive(Parser)]
#[command(
    version,
    about,
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(clap::Arg::new("dummy")
        .value_parser([PossibleValue::new("shuttle")])
        .required(false)
        .hide(true))
)]
pub struct Args {
    /// Run this command against the API at the supplied URL
    /// (allows targeting a custom deployed instance for this command only)
    #[arg(long, env = "SHUTTLE_API")]
    pub api_url: Option<String>,
    #[command(flatten)]
    pub project_args: ProjectArgs,
    #[command(subcommand)]
    pub cmd: Command,
}

// Common args for subcommands that deal with projects.
#[derive(Parser, Debug)]
pub struct ProjectArgs {
    /// Specify the working directory
    #[arg(global = true, long, default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_path))]
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

#[derive(Parser)]
pub enum Command {
    /// Deploy a shuttle service
    Deploy(DeployArgs),
    /// Manage deployments of a shuttle service
    #[command(subcommand)]
    Deployment(DeploymentCommand),
    /// Manage resources of a shuttle project
    #[command(subcommand)]
    Resource(ResourceCommand),
    /// Create a new shuttle project
    Init(InitArgs),
    /// Generate shell completions
    Generate {
        /// Which shell
        #[arg(short, long, env, default_value_t = Shell::Bash)]
        shell: Shell,
        /// Output to a file (stdout by default)
        #[arg(short, long, env)]
        output: Option<PathBuf>,
    },
    /// View the status of a shuttle service
    Status,
    /// View the logs of a deployment in this shuttle service
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
    /// Remove cargo build artifacts in the shuttle environment
    Clean,
    /// Stop this shuttle service
    Stop,
    /// Manage secrets for this shuttle service
    Secrets,
    /// Login to the shuttle platform
    Login(LoginArgs),
    /// Log out of the shuttle platform
    Logout,
    /// Run a shuttle service locally
    Run(RunArgs),
    /// Open an issue on GitHub and provide feedback
    Feedback,
    /// List or manage projects on shuttle
    #[command(subcommand)]
    Project(ProjectCommand),
}

#[derive(Parser)]
pub enum DeploymentCommand {
    /// List all the deployments for a service
    List,
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
}

#[derive(Parser)]
pub enum ProjectCommand {
    /// Create an environment for this project on shuttle
    Start {
        #[arg(long, default_value_t = IDLE_MINUTES)]
        /// How long to wait before putting the project in an idle state due to inactivity. 0 means the project will never idle
        idle_minutes: u64,
    },
    /// Check the status of this project's environment on shuttle
    Status {
        #[arg(short, long)]
        /// Follow status of project command
        follow: bool,
    },
    /// Destroy this project's environment (container) on shuttle
    Stop,
    /// Destroy and create an environment for this project on shuttle
    Restart {
        #[arg(long, default_value_t = IDLE_MINUTES)]
        /// How long to wait before putting the project in an idle state due to inactivity. 0 means the project will never idle
        idle_minutes: u64,
    },
    /// List all projects belonging to the calling account
    List {
        #[arg(long)]
        /// Return projects filtered by a given project status
        filter: Option<String>,
    },
}

#[derive(Parser, Clone, Debug)]
pub struct LoginArgs {
    /// API key for the shuttle platform
    #[arg(long)]
    pub api_key: Option<String>,
}

#[derive(Parser)]
pub struct DeployArgs {
    /// Allow deployment with uncommited files
    #[arg(long)]
    pub allow_dirty: bool,
    /// Don't run pre-deploy tests
    #[arg(long)]
    pub no_test: bool,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Port to start service on
    #[arg(long, env, default_value = "8000")]
    pub port: u16,
    /// Use 0.0.0.0 instead of localhost (for usage with local external devices)
    #[arg(long)]
    pub external: bool,
    /// Use release mode for building the project.
    #[arg(long, short = 'r')]
    pub release: bool,
}

#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Initialize with actix-web framework
    #[arg(long="actix_web", conflicts_with_all = &["axum", "rocket", "tide", "tower", "poem", "serenity", "poise", "warp", "salvo", "thruster", "no_framework"])]
    pub actix_web: bool,
    /// Initialize with axum framework
    #[arg(long, conflicts_with_all = &["actix_web","rocket", "tide", "tower", "poem", "serenity", "poise", "warp", "salvo", "thruster", "no_framework"])]
    pub axum: bool,
    /// Initialize with rocket framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "tide", "tower", "poem", "serenity", "poise", "warp", "salvo", "thruster", "no_framework"])]
    pub rocket: bool,
    /// Initialize with tide framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tower", "poem", "serenity", "poise", "warp", "salvo", "thruster", "no_framework"])]
    pub tide: bool,
    /// Initialize with tower framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "poem", "serenity", "poise", "warp", "salvo", "thruster", "no_framework"])]
    pub tower: bool,
    /// Initialize with poem framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "serenity", "poise", "warp", "salvo", "thruster", "no_framework"])]
    pub poem: bool,
    /// Initialize with salvo framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "poem", "warp", "serenity", "poise", "thruster", "no_framework"])]
    pub salvo: bool,
    /// Initialize with serenity framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "poem", "warp", "poise", "salvo", "thruster", "no_framework"])]
    pub serenity: bool,
    /// Initialize with poise framework
    #[clap(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "poem", "warp", "serenity", "salvo", "thruster", "no_framework"])]
    pub poise: bool,
    /// Initialize with warp framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "poem", "serenity", "poise", "salvo", "thruster", "no_framework"])]
    pub warp: bool,
    /// Initialize with thruster framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "poem", "warp", "salvo", "serenity", "poise", "no_framework"])]
    pub thruster: bool,
    /// Initialize without a framework
    #[arg(long, conflicts_with_all = &["actix_web","axum", "rocket", "tide", "tower", "poem", "warp", "salvo", "serenity", "poise", "thruster"])]
    pub no_framework: bool,
    /// Whether to create the environment for this project on shuttle
    #[arg(long)]
    pub new: bool,
    #[command(flatten)]
    pub login_args: LoginArgs,
    /// Path to initialize a new shuttle project
    #[arg(default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_init_path) )]
    pub path: PathBuf,
}

impl InitArgs {
    pub fn framework(&self) -> Option<Framework> {
        if self.actix_web {
            Some(Framework::ActixWeb)
        } else if self.axum {
            Some(Framework::Axum)
        } else if self.rocket {
            Some(Framework::Rocket)
        } else if self.tide {
            Some(Framework::Tide)
        } else if self.tower {
            Some(Framework::Tower)
        } else if self.poem {
            Some(Framework::Poem)
        } else if self.salvo {
            Some(Framework::Salvo)
        } else if self.poise {
            Some(Framework::Poise)
        } else if self.serenity {
            Some(Framework::Serenity)
        } else if self.warp {
            Some(Framework::Warp)
        } else if self.thruster {
            Some(Framework::Thruster)
        } else if self.no_framework {
            Some(Framework::None)
        } else {
            None
        }
    }
}

// Helper function to parse and return the absolute path
fn parse_path(path: OsString) -> Result<PathBuf, String> {
    dunce::canonicalize(&path).map_err(|e| format!("could not turn {path:?} into a real path: {e}"))
}

// Helper function to parse, create if not exists, and return the absolute path
pub(crate) fn parse_init_path(path: OsString) -> Result<PathBuf, io::Error> {
    // Create the directory if does not exist
    create_dir_all(&path)?;

    parse_path(path.clone()).map_err(|e| {
        io::Error::new(
            ErrorKind::InvalidInput,
            format!("could not turn {path:?} into a real path: {e}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use crate::tests::path_from_workspace_root;

    use super::*;

    fn init_args_factory(framework: &str) -> InitArgs {
        let mut init_args = InitArgs {
            actix_web: false,
            axum: false,
            rocket: false,
            tide: false,
            tower: false,
            poem: false,
            salvo: false,
            serenity: false,
            poise: false,
            warp: false,
            thruster: false,
            no_framework: false,
            new: false,
            login_args: LoginArgs { api_key: None },
            path: PathBuf::new(),
        };

        match framework {
            "actix-web" => init_args.actix_web = true,
            "axum" => init_args.axum = true,
            "rocket" => init_args.rocket = true,
            "tide" => init_args.tide = true,
            "tower" => init_args.tower = true,
            "poem" => init_args.poem = true,
            "salvo" => init_args.salvo = true,
            "serenity" => init_args.serenity = true,
            "poise" => init_args.poise = true,
            "warp" => init_args.warp = true,
            "thruster" => init_args.thruster = true,
            "none" => init_args.no_framework = true,
            _ => unreachable!(),
        }

        init_args
    }

    #[test]
    fn test_init_args_framework() {
        for framework in Framework::iter() {
            let args = init_args_factory(&framework.to_string());
            assert_eq!(args.framework(), Some(framework));
        }
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
