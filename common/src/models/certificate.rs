use comfy_table::{
    presets::{NOTHING, UTF8_BORDERS_ONLY},
    Attribute, Cell, ContentArrangement, Table,
};
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

#[derive(Deserialize, Serialize, Debug)]
#[typeshare::typeshare]
pub struct CertificateListResponse {
    pub certificates: Vec<CertificateResponse>,
}

pub fn get_certificates_table_beta(certs: &[CertificateResponse], raw: bool) -> String {
    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Certificate ID", "Subject", "Expires"]);

    for cert in certs {
        table.add_row(vec![
            Cell::new(&cert.id).add_attribute(Attribute::Bold),
            Cell::new(&cert.subject),
            Cell::new(&cert.not_after),
        ]);
    }

    table.to_string()
}
