use anyhow::{Context, Result};
use lib::{DeploymentMeta, DeploymentStateMeta, ProjectConfig, UNVEIL_PROJECT_HEADER};
use std::{fs::File, thread::sleep, time::Duration};

pub(crate) type ApiKey = String;

pub(crate) fn deploy(package_file: File, api_key: ApiKey, project: ProjectConfig) -> Result<()> {
    let mut url = get_url().to_string();
    url.push_str("/deployments");
    let client = reqwest::blocking::Client::new();
    // example from Stripe:
    // curl https://api.stripe.com/v1/charges -u sk_test_BQokikJOvBiI2HlWgH4olfQ2:

    let mut res: DeploymentMeta = client
        .post(url.clone())
        .body(package_file)
        .header(UNVEIL_PROJECT_HEADER, serde_json::to_string(&project)?)
        .header("Host", "unveil.sh")
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .context("failed to send deployment to the Unveil server")?
        .json()
        .context("failed to parse Unveil response")?;

    url.push_str(&format!("/{}", res.id));
    let mut log_pos = 0;

    while !matches!(
        res.state,
        DeploymentStateMeta::DEPLOYED | DeploymentStateMeta::ERROR
    ) {
        print_log(res.build_logs, &mut log_pos);

        sleep(Duration::from_millis(350));

        res = client
            .get(url.clone())
            .header("Host", "unveil.sh")
            .basic_auth(api_key.clone(), Some(""))
            .send()
            .context("failed to get deployment from the Unveil server")?
            .json()
            .context("failed to parse Unveil response")?;
    }

    print_log(res.build_logs, &mut log_pos);

    Ok(())
}

fn print_log(logs: Option<String>, log_pos: &mut usize) {
    if let Some(logs) = logs {
        let new = &logs[*log_pos..];

        if !new.is_empty() {
            *log_pos = logs.len();
            print!("{}", new);
        }
    }
}

#[cfg(debug_assertions)]
fn get_url() -> &'static str {
    "http://localhost:8000"
}

#[cfg(not(debug_assertions))]
fn get_url() -> &'static str {
    "https://unveil.sh"
}
