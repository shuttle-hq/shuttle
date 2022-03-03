#[macro_use]
extern crate rocket;

mod args;
mod build;
mod deployment;
mod factory;
mod proxy;
mod router;
mod auth;

use factory::UnveilFactory;
use lib::{DeploymentApiError, Port, ProjectConfig};
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Value;
use rocket::{Data, State, tokio};
use std::net::IpAddr;
use std::sync::Arc;
use structopt::StructOpt;

use crate::args::Args;
use crate::build::{BuildSystem, FsBuildSystem};
use crate::deployment::DeploymentSystem;
use crate::auth::User;


/// Status API to be used to check if the service is alive
#[get("/status")]
async fn status() -> () {
    ()
}

#[get("/projects/<project_name>")]
async fn get_project(state: &State<ApiState>, project_name: String, user: User) -> Result<Value, DeploymentApiError> {
    if project_name != user.project_name {
        return Err(DeploymentApiError::NotFound(format!("could not find project `{}`", &project_name)));
    }
    let deployment = state.deployment_manager.get_deployment_for_project(&project_name).await?;
    Ok(json!(deployment))
}

#[delete("/projects/<project_name>")]
async fn delete_project(state: &State<ApiState>, project_name: String, user: User) -> Result<Value, DeploymentApiError> {
    if project_name != user.project_name {
        return Err(DeploymentApiError::NotFound(format!("could not find project `{}`", &project_name)));
    }

    let deployment = state.deployment_manager.kill_deployment_for_project(&project_name).await?;

    Ok(json!(deployment))
}

#[post("/projects", data = "<crate_file>")]
async fn create_project(
    state: &State<ApiState>,
    crate_file: Data<'_>,
    project: ProjectConfig,
    user: User
) -> Result<Value, DeploymentApiError> {
    if project.name != user.project_name {
        return Err(DeploymentApiError::NotFound(format!("could not find project `{}`", &project.name)));
    }

    let deployment = state.deployment_manager.deploy(crate_file, &project).await?;
    Ok(json!(deployment))
}

struct ApiState {
    deployment_manager: Arc<DeploymentSystem>,
}

//noinspection ALL
#[launch]
async fn rocket() -> _ {
    env_logger::Builder::new()
        .filter_module("rocket", log::LevelFilter::Info)
        .filter_module("_", log::LevelFilter::Info)
        .init();

    let args: Args = Args::from_args();
    let build_system = FsBuildSystem::initialise(args.path).unwrap();
    let factory = UnveilFactory {};
    let deployment_manager =
        Arc::new(DeploymentSystem::new(Box::new(build_system), Box::new(factory)).await);

    start_proxy(args.bind_addr, args.proxy_port, deployment_manager.clone()).await;

    let state = ApiState { deployment_manager };

    let config = rocket::Config {
        address: args.bind_addr,
        port: args.api_port,
        ..Default::default()
    };
    rocket::custom(config)
        .mount(
            "/",
            routes![delete_project, create_project, get_project, status],
        )
        .manage(state)
}

async fn start_proxy(
    bind_addr: IpAddr,
    proxy_port: Port,
    deployment_manager: Arc<DeploymentSystem>,
) {
    tokio::spawn(async move { proxy::start(bind_addr, proxy_port, deployment_manager).await });
}
