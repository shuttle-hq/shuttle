use std::path::PathBuf;

use clap::{
    builder::{OsStringValueParser, TypedValueParser},
    Args, Parser, Subcommand,
};

use crate::args::{parse_path, OutputMode};

#[derive(Parser)]
#[command(version, next_help_heading = "Global options")]
pub struct NeptuneArgs {
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

    #[command(subcommand)]
    pub cmd: NeptuneCommand,
}

#[allow(rustdoc::bare_urls)]
/// CLI for the Shuttle platform (https://www.shuttle.dev/)
///
/// See the CLI docs for more information: https://docs.shuttle.dev/guides/cli
#[derive(Subcommand)]
pub enum NeptuneCommand {
    // /// Generate a Shuttle project from a template
    // #[command(visible_alias = "i")]
    // Init(InitArgs),
    // /// Run a project locally
    // #[command(visible_alias = "r")]
    // Run(RunArgs),
    /// Build a project
    #[command(visible_alias = "b")]
    Build(BuildArgs),
    // /// Deploy a project
    // #[command(visible_alias = "d")]
    // Deploy(DeployArgs),
    // /// Show info about your Shuttle account
    // #[command(visible_alias = "acc")]
    // Account,
    // /// Log in to the Shuttle platform
    // Login(LoginArgs),
    // /// Log out of the Shuttle platform
    // Logout(LogoutArgs),
    // /// Generate shell completions and man page
    // #[command(subcommand)]
    // Generate(GenerateCommand),
    // /// Upgrade the Shuttle CLI binary
    // Upgrade {
    //     /// Install an unreleased version from the repository's main branch
    //     #[arg(long)]
    //     preview: bool,
    // },
    // /// Commands for the Shuttle MCP server
    // #[command(subcommand)]
    // Mcp(McpCommand),
}

#[derive(Args, Debug, Default)]
pub struct BuildArgs {}
