use std::collections::HashMap;
use std::fmt::Write;
use std::fs::File;
use std::io::Read;

use anyhow::{anyhow, Context, Result};
use crossterm::style::Stylize;
use futures::StreamExt;
use reqwest::{Response, StatusCode};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::policies::ExponentialBackoff;
use reqwest_retry::RetryTransientMiddleware;
use shuttle_common::project::ProjectName;
use shuttle_common::{deployment, service, ApiKey, ApiUrl, SHUTTLE_PROJECT_HEADER};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::error;

pub(crate) async fn auth(mut api_url: ApiUrl, username: String) -> Result<ApiKey> {
    let client = get_retry_client();

    let _ = write!(api_url, "/users/{}", username);

    let res: Response = client
        .post(api_url)
        .send()
        .await
        .context("failed to get API key from Shuttle server")?;

    let response_status = res.status();
    let response_text = res.text().await?;

    if response_status == StatusCode::OK {
        return Ok(response_text);
    }

    error!(
        text = response_text,
        status = %response_status,
        "failed to authenticate with server"
    );
    Err(anyhow!("failed to authenticate with server",))
}

pub(crate) async fn delete(
    mut api_url: ApiUrl,
    api_key: &ApiKey,
    project: &ProjectName,
) -> Result<()> {
    let client = get_retry_client();

    let _ = write!(api_url, "/services/{}", project);
    let res: Response = client
        .delete(api_url)
        .basic_auth(api_key, Some(""))
        .send()
        .await
        .context("failed to delete service on the Shuttle server")?;

    let service = to_api_result(res).await?;

    println!(
        r#"{}
{}"#,
        "Successfully deleted service".bold(),
        service
    );

    Ok(())
}

pub(crate) async fn status(
    mut api_url: ApiUrl,
    api_key: &ApiKey,
    project: &ProjectName,
) -> Result<()> {
    let client = get_retry_client();

    let _ = write!(api_url, "/services/{}", project);

    let res: Response = client
        .get(api_url)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to get deployment metadata")?;

    let service = to_api_result(res).await?;

    println!("{}", service);

    Ok(())
}

pub(crate) async fn shuttle_version(mut api_url: ApiUrl) -> Result<String> {
    let client = get_retry_client();
    api_url.push_str("/version");

    let res: Response = client
        .get(api_url)
        .send()
        .await
        .context("failed to get version from Shuttle server")?;

    let response_status = res.status();

    if response_status == StatusCode::OK {
        Ok(res.text().await?)
    } else {
        error!(
            text = res.text().await?,
            status = %response_status,
            "failed to get shuttle version from server"
        );
        Err(anyhow!("failed to get shuttle version from server"))
    }
}

pub(crate) async fn logs(api_url: ApiUrl, api_key: &ApiKey, project: &ProjectName) -> Result<()> {
    let client = get_retry_client();

    let deployment_meta = get_deployment_meta(api_url, api_key, project, &client).await?;

    for (datetime, log_item) in deployment_meta.runtime_logs {
        print::log(datetime, log_item);
    }

    Ok(())
}

async fn get_deployment_meta(
    mut api_url: ApiUrl,
    api_key: &ApiKey,
    project: &ProjectName,
    client: &ClientWithMiddleware,
) -> Result<DeploymentMeta> {
    let _ = write!(api_url, "/projects/{}", project);

    let res: Response = client
        .get(api_url)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to get deployment metadata")?;

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
    api_url: ApiUrl,
    api_key: &ApiKey,
    project: &ProjectName,
) -> Result<deployment::State> {
    let mut url = api_url.clone();
    let _ = write!(url, "/services/{}", project.as_str());

    let client = get_retry_client();

    let mut package_file = package_file;
    let mut package_content = Vec::new();
    package_file
        .read_to_end(&mut package_content)
        .context("failed to convert package content to buf")?;

    let res: Response = client
        .post(url)
        .body(package_content)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to send deployment to the Shuttle server")?;

    let text = res.text().await?;
    let res = serde_json::from_str::<deployment::Response>(&text).with_context(|| {
        error!(text, "failed to parse deployment response");
        "could not parse server response"
    })?;

    println!("");
    println!("{res}");

    let id = res.id;
    let mut ws_url = api_url.clone().replace("http", "ws");
    let _ = write!(ws_url, "/deployments/{}/build-logs-subscribe", id);

    let (mut stream, _) = connect_async(ws_url).await.with_context(|| {
        error!("failed to connect to build logs websocket");
        "could not connect to build logs websocket"
    })?;

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(line) => println!("{line}"),
            _ => {}
        }
    }

    status(api_url, api_key, project).await?;

    Ok(res.state)
}

pub(crate) async fn secrets(
    mut api_url: ApiUrl,
    api_key: &ApiKey,
    project: &ProjectName,
    secrets: HashMap<String, String>,
) -> Result<()> {
    if secrets.is_empty() {
        return Ok(());
    }

    let _ = write!(api_url, "/projects/{}/secrets/", project.as_str());

    let client = get_retry_client();

    client
        .post(api_url)
        .body(serde_json::to_string(&secrets)?)
        .header(SHUTTLE_PROJECT_HEADER, serde_json::to_string(&project)?)
        .basic_auth(api_key.clone(), Some(""))
        .send()
        .await
        .context("failed to send deployment's secrets to the Shuttle server")
        .map(|_| ())
}

async fn to_api_result(res: Response) -> Result<service::Response> {
    let text = res.text().await?;
    serde_json::from_str::<service::Response>(&text).with_context(|| {
        error!(text, "failed to parse service data");
        "could not parse server response"
    })
}
