use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

use clap::Parser;
use fqdn::FQDN;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// Address to bind provisioner on
    #[clap(long, env = "PROVISIONER_IP", default_value_t = Ipv4Addr::LOCALHOST.into())]
    pub ip: IpAddr,

    /// Port to start provisioner on
    #[clap(long, env = "PROVISIONER_PORT", default_value_t = 5001)]
    pub port: u16,

    /// URI to connect to Postgres for managing shared DB resources
    #[clap(long, env = "PROVISIONER_PG_URI", hide_env_values = true)]
    pub shared_pg_uri: String,

    /// URI to connect to MongoDb for managing shared DB resources
    #[clap(long, env = "PROVISIONER_MONGODB_URI", hide_env_values = true)]
    pub shared_mongodb_uri: String,

    /// Fully qualified domain name this provisioner instance is reachable at
    #[clap(long, env = "PROVISIONER_FQDN", parse(try_from_str = parse_fqdn))]
    pub fqdn: FQDN,

    /// Address the provisioned PostgreSQL DB can be reached at on the internal network
    #[clap(long, env = "PROVISIONER_PG_ADDRESS", default_value = "pg")]
    pub internal_pg_address: String,

    /// Address the provisioned MongoDB can be reached at on the internal network
    #[clap(long, env = "PROVISIONER_MONGODB_ADDRESS", default_value = "mongodb")]
    pub internal_mongodb_address: String,
}

fn parse_fqdn(src: &str) -> Result<FQDN, String> {
    FQDN::from_str(src).map_err(|e| format!("{e:?}"))
}
