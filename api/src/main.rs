#[macro_use]
extern crate rocket;

#[macro_use]
extern crate log;

mod args;
mod auth;
mod auth_admin;
mod build;
mod database;
mod deployment;
mod factory;
mod proxy;
mod router;

use std::net::IpAddr;
use std::sync::Arc;

use auth_admin::Admin;
use deployment::MAX_DEPLOYS;
use factory::ShuttleFactory;
use rocket::serde::json::Json;
use rocket::{tokio, Build, Data, Rocket, State};
use shuttle_common::project::ProjectName;
use shuttle_common::{DeploymentApiError, DeploymentMeta, Port};
use structopt::StructOpt;
use uuid::Uuid;

use crate::args::Args;
use crate::auth::{ApiKey, AuthorizationError, ScopedUser, User, UserDirectory};
use crate::build::{BuildSystem, FsBuildSystem};
use crate::deployment::DeploymentSystem;

type ApiResult<T, E> = Result<Json<T>, E>;

/// Find user by username and return it's API Key.
/// if user does not exist create it and update `users` state to `users.toml`.
/// Finally return user's API Key.
#[post("/users/<username>")]
async fn get_or_create_user(
    user_directory: &State<UserDirectory>,
    username: String,
    _admin: Admin,
) -> Result<ApiKey, AuthorizationError> {
    user_directory.get_or_create(username)
}

/// Status API to be used to check if the service is alive
#[get("/status")]
async fn status() {}

#[get("/<_>/deployments/<id>")]
async fn get_deployment(
    state: &State<ApiState>,
    id: Uuid,
    _user: ScopedUser,
) -> ApiResult<DeploymentMeta, DeploymentApiError> {
    info!("[GET_DEPLOYMENT, {}, {}]", _user.name(), _user.scope());
    let deployment = state.deployment_manager.get_deployment(&id).await?;
    Ok(Json(deployment))
}

#[delete("/<_>/deployments/<id>")]
async fn delete_deployment(
    state: &State<ApiState>,
    id: Uuid,
    _user: ScopedUser,
) -> ApiResult<DeploymentMeta, DeploymentApiError> {
    info!("[DELETE_DEPLOYMENT, {}, {}]", _user.name(), _user.scope());
    // TODO why twice?
    let _deployment = state.deployment_manager.get_deployment(&id).await?;
    let deployment = state.deployment_manager.kill_deployment(&id).await?;
    Ok(Json(deployment))
}

#[get("/<_>")]
async fn get_project(
    state: &State<ApiState>,
    user: ScopedUser,
) -> ApiResult<DeploymentMeta, DeploymentApiError> {
    info!("[GET_PROJECT, {}, {}]", user.name(), user.scope());

    let deployment = state
        .deployment_manager
        .get_deployment_for_project(user.scope())
        .await?;

    Ok(Json(deployment))
}

#[delete("/<_>")]
async fn delete_project(
    state: &State<ApiState>,
    user: ScopedUser,
) -> ApiResult<DeploymentMeta, DeploymentApiError> {
    info!("[DELETE_PROJECT, {}, {}]", user.name(), user.scope());

    let deployment = state
        .deployment_manager
        .kill_deployment_for_project(user.scope())
        .await?;
    Ok(Json(deployment))
}

#[post("/<project_name>", data = "<crate_file>")]
async fn create_project(
    state: &State<ApiState>,
    user_directory: &State<UserDirectory>,
    crate_file: Data<'_>,
    project_name: ProjectName,
    user: User,
) -> ApiResult<DeploymentMeta, DeploymentApiError> {
    info!("[CREATE_PROJECT, {}, {}]", &user.name, &project_name);

    if !user
        .projects
        .iter()
        .any(|my_project| *my_project == project_name)
    {
        user_directory.create_project_if_not_exists(&user.name, &project_name)?;
    }
    let deployment = state
        .deployment_manager
        .deploy(crate_file, project_name)
        .await?;
    Ok(Json(deployment))
}

struct ApiState {
    deployment_manager: Arc<DeploymentSystem>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(MAX_DEPLOYS)
        .build()
        .unwrap()
        .block_on(async {
            rocket().await.launch().await?;

            Ok(())
        })
}

//noinspection ALL
async fn rocket() -> Rocket<Build> {
    env_logger::Builder::new()
        .filter_module("rocket", log::LevelFilter::Warn)
        .filter_module("_", log::LevelFilter::Warn)
        .filter_module("api", log::LevelFilter::Debug)
        .init();

    let args: Args = Args::from_args();
    let build_system = FsBuildSystem::initialise(args.path).unwrap();
    let deployment_manager =
        Arc::new(DeploymentSystem::new(Box::new(build_system), args.proxy_fqdn.to_string()).await);

    start_proxy(args.bind_addr, args.proxy_port, deployment_manager.clone()).await;

    let state = ApiState { deployment_manager };

    let user_directory =
        UserDirectory::from_user_file().expect("could not initialise user directory");

    let config = rocket::Config {
        address: args.bind_addr,
        port: args.api_port,
        ..Default::default()
    };
    rocket::custom(config)
        .mount(
            "/projects",
            routes![
                delete_deployment,
                get_deployment,
                delete_project,
                create_project,
                get_project,
            ],
        )
        .mount("/", routes![get_or_create_user, status])
        .manage(state)
        .manage(user_directory)
}

async fn start_proxy(
    bind_addr: IpAddr,
    proxy_port: Port,
    deployment_manager: Arc<DeploymentSystem>,
) {
    tokio::spawn(async move { proxy::start(bind_addr, proxy_port, deployment_manager).await });
}
