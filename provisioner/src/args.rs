use std::net::{IpAddr, Ipv4Addr};

use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Address to bind provisioner on
    #[clap(short, long, env = "PROVISIONER_IP", default_value_t = Ipv4Addr::LOCALHOST.into())]
    pub ip: IpAddr,

    /// Port to start provisioner on
    #[clap(short, long, env = "PROVISIONER_PORT", default_value_t = 5001)]
    pub port: u16,

    /// URI to connect to Postgres for managing shared DB resources
    #[clap(short, long, env = "PROVISIONER_PG_URI", hide_env_values = true)]
    pub shared_pg_uri: String,
}
