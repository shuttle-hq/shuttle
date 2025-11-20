use std::path::PathBuf;

use cargo_shuttle::args::{
    parse_and_create_path, parse_path, InitTemplateArg, OutputMode, TemplateLocation,
};
use clap::{
    builder::{OsStringValueParser, TypedValueParser},
    Args, Parser, Subcommand, ValueEnum,
};
use clap_complete::Shell;

use crate::config::NeptuneConfig;

#[derive(Parser)]
#[command(version)]
pub struct NeptuneArgs {
    #[command(flatten)]
    pub globals: NeptuneGlobalArgs,

    #[command(subcommand)]
    pub cmd: NeptuneCommand,
}

#[derive(Args, Clone)]
#[command(next_help_heading = "Global options")]
pub struct NeptuneGlobalArgs {
    /// URL for the Neptune API to target
    #[arg(global = true, long, env = "NEPTUNE_API", hide = true)]
    pub api_url: Option<String>,
    /// URL for the Neptune AI service to target
    #[arg(global = true, long, env = "NEPTUNE_AI", hide = true)]
    pub ai_url: Option<String>,
    /// Neptune API key
    #[arg(global = true, long, env = "NEPTUNE_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
    /// Turn on tracing output for Shuttle libraries. (WARNING: can print sensitive data)
    #[arg(global = true, long, env = "NEPTUNE_DEBUG")]
    pub debug: bool,
    /// What format to print output in
    #[arg(
        global = true,
        long = "output",
        env = "NEPTUNE_OUTPUT_MODE",
        default_value = "normal"
    )]
    pub output_mode: OutputMode,

    /// Utility for knowing which of the above config fields were given as args, not used for parsing
    #[arg(skip)]
    pub arg_provided_fields: Vec<&'static str>,

    // Global args that can't be modified in config:
    #[arg(global = true, long, visible_alias = "wd", default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_path))]
    pub working_directory: PathBuf,

    #[arg(global = true, long, short = 'v', env = "NEPTUNE_VERBOSE")]
    pub verbose: bool,
    /// Ignore blocking AI lint errors (not recommended)
    #[arg(global = true, long)]
    pub allow_ai_errors: bool,
    /// Ignore blocking AI lint warnings even if block_on_warnings is set
    #[arg(global = true, long)]
    pub allow_ai_warnings: bool,
}

impl NeptuneGlobalArgs {
    pub fn workdir_name(&self) -> Option<String> {
        self.working_directory
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
    }

    pub fn into_config(self) -> NeptuneConfig {
        // For args that have default values in clap:
        //   Only set them to Some() if a value was given on the command line,
        //   so that the default value is not mistaken as an explicitly given arg and overrides config from files.
        NeptuneConfig {
            api_url: self.api_url,
            ai_url: self.ai_url,
            api_key: self.api_key,
            debug: self
                .arg_provided_fields
                .contains(&"debug")
                .then_some(self.debug),
            output_mode: self
                .arg_provided_fields
                .contains(&"output_mode")
                .then_some(self.output_mode),
        }
    }
}

#[allow(rustdoc::bare_urls)]
/// CLI for the Neptune platform (https://www.neptune.dev/)
///
/// See the CLI docs for more information: https://docs.shuttle.dev/guides/cli
#[derive(Subcommand)]
pub enum NeptuneCommand {
    /// Generate an Neptune project from a template
    #[command(visible_alias = "i")]
    Init(InitArgs),
    // /// Run a project locally
    // #[command(visible_alias = "r")]
    // Run(RunArgs),
    /// Build a project
    // #[command(visible_alias = "b")]
    // Build(BuildArgs),
    /// Deploy a project
    #[command(visible_alias = "d")]
    Deploy(DeployArgs),
    // /// Show info about your Neptune account
    // #[command(visible_alias = "acc")]
    // Account,
    /// Log in to the Neptune platform
    Login(LoginArgs),
    /// Log out of the Neptune platform
    Logout(LogoutArgs),
    /// Generate AI instructions, shell completions, man page, etc
    #[command(subcommand, visible_alias = "gen")]
    Generate(GenerateCommand),
    /// Upgrade the Neptune CLI binary
    Upgrade {
        /// Install from the repository's main branch (requires `cargo`)
        #[arg(long, hide = true)]
        preview: bool,
    },
    /// List things in your Neptune account
    #[command(visible_alias = "ls")]
    List(ListArgs),
    // /// Commands for the Shuttle MCP server
    // #[command(subcommand)]
    // Mcp(McpCommand),
    /// Get the status of a project
    #[command(visible_alias = "s")]
    Status(StatusArgs),
    /// Delete a project
    #[command(visible_alias = "del")]
    Delete,
    /// Run the AI linter against the current project
    Lint,
    /// Fetch the ProjectSpec JSON schema
    Schema,
}

#[derive(Args, Clone, Debug, Default)]
pub struct StatusArgs {
    /// Explicit project name to fetch status for
    #[arg(long)]
    pub project_name: Option<String>,
}

// #[derive(Args, Debug, Default)]
// pub struct RunArgs {
//     /// Port to start service on
//     #[arg(long, short = 'p', default_value = "8000")]
//     pub port: u16,
//     /// Use 0.0.0.0 instead of localhost
//     #[arg(long)]
//     pub external: bool,
// }

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

    /// Don't initialize a new git repository
    #[arg(long)]
    pub no_git: bool,

    /// Path where to place the new Shuttle project
    #[arg(default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_and_create_path))]
    pub path: PathBuf,

    /// Utility for knowing if the path arg was given on the command line
    #[arg(skip)]
    pub path_provided_arg: bool,
}

impl InitArgs {
    /// Turns the git args to a repo+folder, if present.
    pub fn git_template(&self) -> anyhow::Result<Option<TemplateLocation>> {
        if self.template.is_some() {
            anyhow::bail!("Template arg not yet supported.");
        }
        Ok(self.from.as_ref().map(|from| TemplateLocation {
            auto_path: from.clone(),
            subfolder: self.subfolder.clone(),
        }))
    }
}

#[derive(Args, Default)]
pub struct DeployArgs {
    /// Tag for the docker image
    #[arg(long)]
    pub tag: Option<String>,
    /// Emit the generated Dockerfile to the working directory
    #[arg(long)]
    pub emit_dockerfile: bool,
    /// Skip spec generation and reuse the existing neptune.json
    #[arg(long = "skip-spec")]
    pub skip_spec: bool,
    /// Skip AI lint before deploying
    #[arg(long = "skip-lint")]
    pub skip_lint: bool,
    /// Provide environment variables to your build
    #[arg(long, short)]
    pub env: Vec<String>,
    // /// Use a local Dockerfile instead of Nixpacks
    // #[arg(long)]
    // pub dockerfile: Option<std::path::PathBuf>,
    // /// WIP: Deploy this Docker image instead of building one
    // #[arg(long, short = 'i', hide = true)]
    // pub image: Option<String>,
    // /// Allow deployment with uncommitted files
    // #[arg(long, visible_alias = "ad")]
    // pub allow_dirty: bool,
    // /// Output the deployment archive to a file instead of sending a deployment request
    // #[arg(long)]
    // pub output_archive: Option<PathBuf>,

    // TODO: combine args from generate, plan, build, etc
}

#[derive(Args, Clone, Debug, Default)]
#[command(next_help_heading = "Login options")]
pub struct LoginArgs {
    // /// Prompt to paste the API key instead of opening the browser
    // #[arg(long, conflicts_with = "api_key", alias = "input")]
    // pub prompt: bool,
    /// Log in with this API key
    #[arg(long)]
    pub api_key: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct LogoutArgs {
    // /// Reset the API key before logging out
    // #[arg(long)]
    // pub reset_api_key: bool,
}

#[derive(Args, Clone, Debug)]
pub struct ListArgs {
    /// What to list (e.g., projects)
    #[arg(value_enum)]
    pub what: ListWhat,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum ListWhat {
    Projects,
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
    Manpage {
        /// Output to a file (stdout by default)
        #[arg(short, long)]
        output_file: Option<PathBuf>,
    },
    /// Generate agents.md, Neptune-tailored instructions for AI code agents
    Agents,
    /// Generate spec, instructions and nixpacks command
    Spec,
}
