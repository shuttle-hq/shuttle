use anyhow::{anyhow, Context, Result};
use shuttle_common::{API_URL, ApiKey, DeploymentMeta, DeploymentStateMeta, SHUTTLE_PROJECT_HEADER};
use shuttle_common::project::ProjectConfig;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::{fs::File, io::Read, thread::sleep, time::Duration};
use reqwest::{Response, StatusCode};

pub(crate) async fn auth(username: String) -> Result<ApiKey> {
    let client = get_retry_client();

    let mut url = API_URL.to_string();
    url.push_str(&format!("/users/{}", username));

    let res: Response = client
        .post(url.clone())
        .send()
        .await
        .context("failed to get API key from Shuttle server")?;

    let response_status = res.status();
    let response_text = res.text().await?;

    if response_status == StatusCode::OK {
        return Ok(response_text);
    }

    Err(anyhow!("{}", response_text))
}

pub(crate) async fn delete(api_key: ApiKey, project: ProjectConfig) -> Result<()> {
    let client = get_retry_client();

    let mut url = API_URL.to_string();
    url.push_str(&format!("/projects/{}", project.name()));
    let res: Response = client
        .delete(url.clone())
        .basic_auth(api_key, Some(""))
        .send()
        .await
        .context("failed to delete deployment on the Shuttle server")?;

    let deployment_meta = to_api_result(res).await?;

    println!("{}", deployment_meta);

    Ok(())
}

pub(crate) async fn status(api_key: ApiKey, project: ProjectConfig) -> Result<()> {
    let client = get_retry_client();

    let deployment_meta = get_deployment_meta(&api_key, &project, &client).await?;

    println!("{}", deployment_meta);

    Ok(())
}

async fn get_deployment_meta(
    api_key: &ApiKey,
    project: &ProjectConfig,
    client: &ClientWithMiddleware,
) -> Result<DeploymentMeta> {
    let mut url = API_URL.to_string();
    url.push_str(&format!("/projects/{}", project.name()));

    let res: Response = client
        .get(url.clone())
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to get deployment from the Shuttle server")?;

    to_api_result(res).await
}

fn get_retry_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

pub(crate) async fn deploy(
    package_file: File,
    api_key: ApiKey,
    project: ProjectConfig,
) -> Result<()> {
    let mut url = API_URL.to_string();
    url.push_str("/projects");

    let client = get_retry_client();

    let mut package_file = package_file;
    let mut package_content = Vec::new();
    package_file
        .read_to_end(&mut package_content)
        .context("failed to convert package content to buf")?;

    let res: Response = client
        .post(url.clone())
        .body(package_content)
        .header(SHUTTLE_PROJECT_HEADER, serde_json::to_string(&project)?)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to send deployment to the Shuttle server")?;

    let mut deployment_meta = to_api_result(res).await?;

    let mut log_pos = 0;

    while !matches!(
        deployment_meta.state,
        DeploymentStateMeta::Deployed | DeploymentStateMeta::Error(_)
    ) {
        print_log(&deployment_meta.build_logs, &mut log_pos);

        sleep(Duration::from_millis(350));

        deployment_meta = get_deployment_meta(&api_key, &project, &client).await?;
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

async fn to_api_result(res: Response) -> Result<DeploymentMeta> {
    let text = res.text().await?;
    match serde_json::from_str::<DeploymentMeta>(&text) {
        Ok(meta) => Ok(meta),
        Err(_) => Err(anyhow!("{}", text))
    }
}