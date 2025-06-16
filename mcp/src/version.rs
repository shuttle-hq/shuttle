const GITHUB_RAW_BASE_URL: &'static str =
    "https://raw.githubusercontent.com/dcodesdev/shuttle/refs/heads/main";

pub async fn check_new_version() -> Result<bool, Box<dyn std::error::Error>> {
    let current_version = env!("CARGO_PKG_VERSION");
    let remote_cargo_toml_url = format!("{}/mcp/Cargo.toml", GITHUB_RAW_BASE_URL);
    let root_cargo_toml_url = format!("{}/Cargo.toml", GITHUB_RAW_BASE_URL);

    let response = reqwest::get(&remote_cargo_toml_url).await?;
    let toml_content = response.text().await?;

    let parsed_toml: toml::Value = toml::from_str(&toml_content)?;

    let remote_version =
        if let Some(version_table) = parsed_toml.get("package").and_then(|p| p.get("version")) {
            if let Some(version_str) = version_table.as_str() {
                version_str.to_string()
            } else if version_table
                .get("workspace")
                .and_then(|w| w.as_bool())
                .unwrap_or(false)
            {
                // Fetch root Cargo.toml for workspace version
                let root_response = reqwest::get(&root_cargo_toml_url).await?;
                let root_toml_content = root_response.text().await?;
                let root_parsed_toml: toml::Value = toml::from_str(&root_toml_content)?;

                root_parsed_toml
                    .get("workspace")
                    .and_then(|w| w.get("package"))
                    .and_then(|p| p.get("version"))
                    .and_then(|v| v.as_str())
                    .ok_or("Failed to extract workspace version from root Cargo.toml")?
                    .to_string()
            } else {
                return Err("Invalid version format in remote Cargo.toml".into());
            }
        } else {
            return Err("Failed to extract version from remote Cargo.toml".into());
        };

    let current = semver::Version::parse(current_version)?;
    let remote = semver::Version::parse(&remote_version)?;

    Ok(remote > current)
}
