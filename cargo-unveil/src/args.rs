use structopt::StructOpt;

#[derive(StructOpt)]
#[structopt(
    // Cargo passes in the subcommand name to the invoked executable. Use a
    // hidden, optional positional argument to deal with it.
    arg(structopt::clap::Arg::with_name("dummy")
        .possible_value("unveil")
        .required(false)
        .hidden(true))
)]
pub enum Args {
    #[structopt(about = "deploy an unveil project")]
    Deploy(DeployArgs)
}

#[derive(StructOpt)]
pub struct DeployArgs {
    #[structopt(long, about = "Allow dirty working directories to be packaged")]
    pub allow_dirty: bool
}