use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[typeshare::typeshare]
pub struct AddCertificateRequest {
    #[serde(alias = "domain")]
    pub subject: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[typeshare::typeshare]
pub struct DeleteCertificateRequest {
    #[serde(alias = "domain")]
    pub subject: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[typeshare::typeshare]
pub struct CertificateResponse {
    pub id: String,
    pub subject: String,
    pub serial_hex: String,
    pub not_after: String,
}
