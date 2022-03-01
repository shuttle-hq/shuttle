use anyhow::{Context, Result};
use lib::{
    DeploymentId, DeploymentMeta, DeploymentStateMeta, ProjectConfig, API_URL,
    UNVEIL_PROJECT_HEADER,
};
use reqwest::blocking::Client;
use std::{fs::File, thread::sleep, time::Duration};

pub(crate) type ApiKey = String;

pub(crate) fn delete(api_key: ApiKey, deployment_id: DeploymentId) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    let mut url = API_URL.to_string();
    url.push_str(&format!("/deployments/{}", deployment_id));
    let deployment_meta: DeploymentMeta = client
        .delete(url.clone())
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .context("failed to delete deployment on the Unveil server")?
        .json()
        .context("failed to parse Unveil response")?;

    println!("{}", deployment_meta);

    Ok(())
}

pub(crate) fn status(api_key: ApiKey, deployment_id: DeploymentId) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    let deployment_meta = get_deployment_meta(&api_key, &deployment_id, &client)?;

    println!("{}", deployment_meta);

    Ok(())
}

fn get_deployment_meta(
    api_key: &ApiKey,
    deployment_id: &DeploymentId,
    client: &Client,
) -> Result<DeploymentMeta> {
    let mut url = API_URL.to_string();
    url.push_str(&format!("/deployments/{}", deployment_id));
    client
        .get(url.clone())
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .context("failed to get deployment from the Unveil server")?
        .json()
        .context("failed to parse Unveil response")
}

pub(crate) fn deploy(package_file: File, api_key: ApiKey, project: ProjectConfig) -> Result<()> {
    let mut url = API_URL.to_string();
    url.push_str("/deployments");
    let client = reqwest::blocking::Client::new();
    // example from Stripe:
    // curl https://api.stripe.com/v1/charges -u sk_test_BQokikJOvBiI2HlWgH4olfQ2:

    let mut deployment_meta: DeploymentMeta = client
        .post(url.clone())
        .body(package_file)
        .header(UNVEIL_PROJECT_HEADER, serde_json::to_string(&project)?)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .context("failed to send deployment to the Unveil server")?
        .json()
        .context("failed to parse Unveil response")?;

    let mut log_pos = 0;

    while !matches!(
        deployment_meta.state,
        DeploymentStateMeta::Deployed | DeploymentStateMeta::Error
    ) {
        print_log(&deployment_meta.build_logs, &mut log_pos);

        sleep(Duration::from_millis(350));

        deployment_meta = get_deployment_meta(&api_key, &deployment_meta.id, &client)?;
    }

    print_log(&deployment_meta.build_logs, &mut log_pos);

    println!("{}", &deployment_meta);

    Ok(())
}

fn print_log(logs: &Option<String>, log_pos: &mut usize) {
    if let Some(logs) = logs {
        let new = &logs[*log_pos..];

        if !new.is_empty() {
            *log_pos = logs.len();
            print!("{}", new);
        }
    }
}
