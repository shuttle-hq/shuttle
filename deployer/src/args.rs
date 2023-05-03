use clap::Parser;
use http::Uri;

/// Service to deploy projects on the shuttle infrastructure
#[derive(Debug, Parser)]
#[clap(author, version, about)]
pub struct Args {
    /// Address to reach the authentication service at
    #[clap(long, default_value = "http://127.0.0.1:8008")]
    pub auth_uri: Uri,
}
