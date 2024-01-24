use std::{
    net::{IpAddr, Ipv4Addr},
    str::FromStr,
};

use clap::Parser;
use fqdn::FQDN;
use tonic::transport::Uri;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Address to bind provisioner on
    #[arg(long, env = "PROVISIONER_IP", default_value_t = Ipv4Addr::LOCALHOST.into())]
    pub ip: IpAddr,

    /// Port to start provisioner on
    #[arg(long, env = "PROVISIONER_PORT", default_value_t = 5001)]
    pub port: u16,

    /// URI to connect to Postgres for managing shared DB resources
    #[arg(long, env = "PROVISIONER_PG_URI", hide_env_values = true)]
    pub shared_pg_uri: String,

    /// URI to connect to MongoDb for managing shared DB resources
    #[arg(long, env = "PROVISIONER_MONGODB_URI", hide_env_values = true)]
    pub shared_mongodb_uri: String,

    /// Fully qualified domain name this provisioner instance is reachable at
    #[arg(long, env = "PROVISIONER_FQDN", value_parser = parse_fqdn)]
    pub fqdn: FQDN,

    /// Address the provisioned PostgreSQL DB can be reached at on the internal network
    #[arg(long, env = "PROVISIONER_PG_ADDRESS", default_value = "pg")]
    pub internal_pg_address: String,

    /// Address the provisioned MongoDB can be reached at on the internal network
    #[arg(long, env = "PROVISIONER_MONGODB_ADDRESS", default_value = "mongodb")]
    pub internal_mongodb_address: String,

    /// Address to reach the authentication service at
    #[arg(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,

    /// Address to reach gateway's control plane at
    #[clap(long, default_value = "http://gateway:8001")]
    pub gateway_uri: Uri,

    /// Address to reach resource-recorder service at
    #[clap(long, default_value = "http://resource-recorder:8000")]
    pub resource_recorder_uri: Uri,
}

fn parse_fqdn(src: &str) -> Result<FQDN, String> {
    FQDN::from_str(src).map_err(|e| format!("{e:?}"))
}
