use lib::DeploymentId;
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
    Deploy(DeployArgs),
    #[structopt(about = "view the status of an unveil deployment")]
    Status(StatusArgs),
    #[structopt(about = "view the status of an unveil deployment")]
    Delete(DeleteArgs),
}

#[derive(StructOpt)]
pub struct StatusArgs {
    #[structopt(about = "the id of the target deployment")]
    pub deployment_id: DeploymentId,
}

#[derive(StructOpt)]
pub struct DeleteArgs {
    #[structopt(about = "the id of the target deployment")]
    pub deployment_id: DeploymentId,
}

#[derive(StructOpt)]
pub struct DeployArgs {
    #[structopt(long, about = "allow dirty working directories to be packaged")]
    pub allow_dirty: bool,
}
