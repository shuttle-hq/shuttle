use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct AddCertificateRequest {
    pub domain: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CertificateResponse {}
