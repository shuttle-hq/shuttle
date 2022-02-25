#[macro_use]
extern crate rocket;

mod build;
mod deployment;
mod args;
mod router;
mod proxy;

use std::sync::Arc;
use rocket::{Data, State, tokio};
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Value;
use uuid::Uuid;
use structopt::StructOpt;

use crate::args::Args;
use crate::build::{BuildSystem, FsBuildSystem};
use crate::deployment::{DeploymentError, DeploymentSystem};
use lib::{Port, ProjectConfig};

#[get("/deployments/<id>")]
async fn get_deployment(state: &State<ApiState>, id: Uuid) -> Result<Value, DeploymentError> {
    let deployment = state.deployment_manager
        .get_deployment(&id)
        .await?;

    Ok(json!(deployment))
}

#[post("/deployments", data = "<crate_file>")]
async fn create_deployment(state: &State<ApiState>, crate_file: Data<'_>, config: ProjectConfig) -> Result<Value, DeploymentError> {
    let deployment = state.deployment_manager
        .deploy(crate_file, &config)
        .await?;

    Ok(json!(deployment))
}

struct ApiState {
    deployment_manager: Arc<DeploymentSystem>,
}

//noinspection ALL
#[launch]
async fn rocket() -> _ {
    let args: Args = Args::from_args();
    let build_system = FsBuildSystem::initialise(args.path).unwrap();
    let deployment_manager = Arc::new(
        DeploymentSystem::new(Box::new(build_system)).await
    );

    start_proxy(8000, 8001, deployment_manager.clone()).await;

    let state = ApiState {
        deployment_manager
    };
    rocket::build()
        .mount("/", routes![create_deployment, get_deployment])
        .manage(state)
}

async fn start_proxy(proxy_port: Port, api_port: Port, deployment_manager: Arc<DeploymentSystem>) {
    tokio::spawn(async move { proxy::start(proxy_port, api_port, deployment_manager).await });
}