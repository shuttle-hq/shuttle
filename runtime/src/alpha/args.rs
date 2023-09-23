use shuttle_service::Environment;
use tonic::transport::{Endpoint, Uri};

use crate::args::args;

args! {
    pub struct Args {
        "--port" => pub port: u16,
        "--provisioner-address" => #[arg(default_value = "http://localhost:3000")] pub provisioner_address: Endpoint,
        "--env" => pub env: Environment,
        "--auth-uri" => #[arg(default_value = "http://127.0.0.1:8008")] pub auth_uri: Uri,
    }
}
