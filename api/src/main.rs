#[macro_use]
extern crate rocket;

mod build;
mod deployment;

use rocket::{Data, State};
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Value;
use uuid::Uuid;

use crate::build::{BuildSystem, FsBuildSystem};
use crate::deployment::{DeploymentError, DeploymentSystem};
use lib::ProjectConfig;

#[get("/deployments/<id>")]
async fn get_deployment(state: &State<ApiState>, id: Uuid) -> Result<Value, DeploymentError> {
    let deployment = state.deployment_manager
        .get_deployment(&id)
        .await?;

    Ok(json!(deployment))
}

#[post("/deployments", data = "<crate_file>")]
async fn create_deployment(state: &State<ApiState>, crate_file: Data<'_>) -> Result<Value, DeploymentError> {
    let project = ProjectConfig {
        name: "some_project".to_string()
    };

    let deployment = state.deployment_manager.deploy(crate_file, &project).await?;

    Ok(json!(deployment))
}

struct ApiState {
    deployment_manager: DeploymentSystem,
}

//noinspection ALL
#[launch]
async fn rocket() -> _ {
    let deployment_manager = DeploymentSystem::new(Box::new(FsBuildSystem)).await;
    let state = ApiState {
        // we probably want to put the Mutex deeper in the object tree.
        // but it's ok for prototype
        deployment_manager: deployment_manager
    };

    rocket::build()
        .mount("/", routes![create_deployment, get_deployment])
        .manage(state)
}
