use std::path::PathBuf;

use clap::{
    builder::{OsStringValueParser, TypedValueParser},
    Args, Parser, Subcommand,
};
use clap_complete::Shell;

use crate::args::{create_and_parse_path, parse_path, InitTemplateArg, OutputMode};

#[derive(Parser)]
#[command(version)]
pub struct ImpulseArgs {
    #[command(flatten)]
    pub globals: ImpulseGlobalArgs,

    #[command(subcommand)]
    pub cmd: ImpulseCommand,
}

#[derive(Args)]
#[command(next_help_heading = "Global options")]
pub struct ImpulseGlobalArgs {
    /// URL for the Shuttle API to target (overrides inferred URL from api_env)
    #[arg(global = true, long, env = "SHUTTLE_API", hide = true)]
    pub api_url: Option<String>,
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

    #[arg(global = true, long, visible_alias = "wd", default_value = ".", value_parser = OsStringValueParser::new().try_map(parse_path))]
    pub working_directory: PathBuf,
}

#[allow(rustdoc::bare_urls)]
/// CLI for the Impulse platform (https://www.shuttle.dev/)
///
/// See the CLI docs for more information: https://docs.shuttle.dev/guides/cli
#[derive(Subcommand)]
pub enum ImpulseCommand {
    /// Generate an Impulse project from a template
    #[command(visible_alias = "i")]
    Init(InitArgs),
    /// Run a project locally
    #[command(visible_alias = "r")]
    Run(RunArgs),
    /// Build a project
    #[command(visible_alias = "b")]
    Build(BuildArgs),
    /// Deploy a project
    #[command(visible_alias = "d")]
    Deploy(DeployArgs),
    // /// Show info about your Impulse account
    // #[command(visible_alias = "acc")]
    // Account,
    /// Log in to the Impulse platform
    Login(LoginArgs),
    /// Log out of the Impulse platform
    Logout(LogoutArgs),
    /// Generate shell completions, man page, AI instructions, etc
    #[command(subcommand)]
    Generate(GenerateCommand),
    /// Upgrade the Impulse CLI binary
    Upgrade {
        /// Install from the repository's main branch (requires `cargo`)
        #[arg(long)]
        preview: bool,
    },
    // /// Commands for the Shuttle MCP server
    // #[command(subcommand)]
    // Mcp(McpCommand),
}

#[derive(Args, Debug, Default)]
pub struct RunArgs {
    /// Port to start service on
    #[arg(long, short = 'p', default_value = "8000")]
    pub port: u16,
    /// Use 0.0.0.0 instead of localhost
    #[arg(long)]
    pub external: bool,
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

    /// Don't initialize a new git repository
    #[arg(long)]
    pub no_git: bool,

    /// Path where to place the new Shuttle project
    #[arg(default_value = ".", value_parser = OsStringValueParser::new().try_map(create_and_parse_path))]
    pub path: PathBuf,
}

#[derive(Args, Debug, Default)]
pub struct BuildArgs {
    pub path: String,
}

#[derive(Args, Default)]
pub struct DeployArgs {
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
    /// Log in with this Shuttle API key
    #[arg(long)]
    pub api_key: Option<String>,
}

#[derive(Args, Clone, Debug)]
pub struct LogoutArgs {
    // /// Reset the API key before logging out
    // #[arg(long)]
    // pub reset_api_key: bool,
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
    /// Generate agents.md, Impulse-tailored instructions for AI code agents
    Agents,
}
