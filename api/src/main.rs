#[macro_use]
extern crate rocket;

mod build;
mod deployment;

use std::sync::Mutex;
use rocket::{Data, State};
use rocket::serde::json::serde_json::json;
use rocket::serde::json::Value;
use uuid::Uuid;

use crate::build::{BuildSystem, FsBuildSystem, ProjectConfig};
use crate::deployment::{DeploymentError, DeploymentSystem};

#[get("/deployments/<id>")]
fn get_deployment(state: &State<ApiState>, id: Uuid) -> Result<Value, DeploymentError> {
    // let deployment = state.deployment_manager.lock()
    //     .map_err(|_| DeploymentError::Internal("an internal error occurred".to_string()))?
    //     .get_deployment(&id)
    //     .ok_or(DeploymentError::NotFound("could not find deployment".to_string()))?;
    //
    // Ok(json!(deployment))
    unimplemented!()
}

#[post("/deployments", data = "<crate_file>")]
fn create_deployment(state: &State<ApiState>, crate_file: Data) -> Result<Value, DeploymentError> {
    let project = ProjectConfig {
        name: "some_project".to_string()
    };

    let deployment = state.deployment_manager.deploy(crate_file, &project)?;

    Ok(json!(deployment))
}

struct ApiState {
    deployment_manager: DeploymentSystem,
}

//noinspection ALL
#[launch]
fn rocket() -> _ {
    let deployment_manager = DeploymentSystem::new(Box::new(FsBuildSystem));
    let state = ApiState {
        // we probably want to put the Mutex deeper in the object tree.
        // but it's ok for prototype
        deployment_manager: deployment_manager
    };

    rocket::build()
        .mount("/", routes![create_deployment, get_deployment])
        .manage(state)
}
