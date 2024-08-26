use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct AddCertificateRequest {
    pub domain: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DeleteCertificateRequest {
    pub domain: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct CertificateResponse {
    pub subject: String,
    pub serial_hex: String,
    pub not_after: String,
}
