use serde::Deserialize;

use crate::mcp::utils::build_client;

#[derive(Deserialize)]
struct CratesIoResponse {
    #[serde(rename = "crate")]
    crate_info: CrateInfo,
}

#[derive(Deserialize)]
struct CrateInfo {
    max_version: String,
}

pub async fn check_new_version() -> Result<bool, Box<dyn std::error::Error>> {
    let current_version = env!("CARGO_PKG_VERSION");
    let crates_io_url = "https://crates.io/api/v1/crates/shuttle-mcp";

    let client = build_client()?;

    let response: CratesIoResponse = client.get(crates_io_url).send().await?.json().await?;

    let current = semver::Version::parse(current_version)?;
    let remote = semver::Version::parse(&response.crate_info.max_version)?;

    Ok(remote > current)
}
