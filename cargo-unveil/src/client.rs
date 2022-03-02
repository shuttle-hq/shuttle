use anyhow::{Context, Result};
use lib::{DeploymentId, DeploymentMeta, DeploymentStateMeta, ProjectConfig, API_URL, UNVEIL_PROJECT_HEADER, ApiKey};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::{fs::File, io::Read, thread::sleep, time::Duration};

pub(crate) async fn delete(api_key: ApiKey, deployment_id: DeploymentId) -> Result<()> {
    let client = get_retry_client();

    let mut url = API_URL.to_string();
    url.push_str(&format!("/deployments/{}", deployment_id));
    let deployment_meta: DeploymentMeta = client
        .delete(url.clone())
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to delete deployment on the Unveil server")?
        .json()
        .await
        .context("failed to parse Unveil response")?;

    println!("{}", deployment_meta);

    Ok(())
}

pub(crate) async fn status(api_key: ApiKey, deployment_id: DeploymentId) -> Result<()> {
    let client = get_retry_client();

    let deployment_meta = get_deployment_meta(&api_key, &deployment_id, &client).await?;

    println!("{}", deployment_meta);

    Ok(())
}

async fn get_deployment_meta(
    api_key: &ApiKey,
    deployment_id: &DeploymentId,
    client: &ClientWithMiddleware,
) -> Result<DeploymentMeta> {
    let mut url = API_URL.to_string();
    url.push_str(&format!("/deployments/{}", deployment_id));
    client
        .get(url.clone())
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to get deployment from the Unveil server")?
        .json()
        .await
        .context("failed to parse Unveil response")
}

fn get_retry_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    let client = ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    client
}

pub(crate) async fn deploy(
    package_file: File,
    api_key: ApiKey,
    project: ProjectConfig,
) -> Result<()> {
    let mut url = API_URL.to_string();
    url.push_str("/deployments");

    let client = get_retry_client();

    let mut package_file = package_file;
    let mut package_content = Vec::new();
    package_file
        .read_to_end(&mut package_content)
        .context("failed to convert package content to buf")?;

    // example from Stripe:
    // curl https://api.stripe.com/v1/charges -u sk_test_BQokikJOvBiI2HlWgH4olfQ2:

    let mut deployment_meta: DeploymentMeta = client
        .post(url.clone())
        .body(package_content)
        .header(UNVEIL_PROJECT_HEADER, serde_json::to_string(&project)?)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to send deployment to the Unveil server")?
        .json()
        .await
        .context("failed to parse Unveil response")?;

    let mut log_pos = 0;

    while !matches!(
        deployment_meta.state,
        DeploymentStateMeta::Deployed | DeploymentStateMeta::Error
    ) {
        print_log(&deployment_meta.build_logs, &mut log_pos);

        sleep(Duration::from_millis(350));

        deployment_meta = get_deployment_meta(&api_key, &deployment_meta.id, &client).await?;
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
