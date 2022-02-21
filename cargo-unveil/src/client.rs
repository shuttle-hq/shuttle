use std::fs::File;
use anyhow::Result;
use crate::config::Project;

pub(crate) type ApiKey = String;
type DeployResult = String;

pub(crate) fn deploy(package_file: File, api_key: ApiKey, project: Project) -> Result<DeployResult> {
    let mut url = get_url().to_string();
    url.push_str("/deploy");
    let client = reqwest::blocking::Client::new();
    // example from Stripe:
    // curl https://api.stripe.com/v1/charges -u sk_test_BQokikJOvBiI2HlWgH4olfQ2:
    client.post(url)
        .body(package_file)
        .basic_auth(api_key, Some(""))
        .send()?;

    Ok("Deployed!".to_string())
}

#[cfg(debug_assertions)]
fn get_url() -> &'static str {
    "http://localhost:8000"
}

#[cfg(not(debug_assertions))]
fn get_url() -> &'static str {
    "https://unveil.sh"
}