use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(structopt::clap::Arg::with_name("dummy")
        .possible_value("shuttle")
        .required(false)
        .hidden(true))
)]
pub enum Args {
    #[structopt(about = "deploy an shuttle project")]
    Deploy(DeployArgs),
    #[structopt(about = "view the status of an shuttle deployment")]
    Status,
    #[structopt(about = "view the status of an shuttle deployment")]
    Delete,
}

#[derive(StructOpt)]
pub struct DeployArgs {
    #[structopt(long, about = "allow dirty working directories to be packaged")]
    pub allow_dirty: bool,
}
