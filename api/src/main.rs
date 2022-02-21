#![feature(proc_macro_hygiene, decl_macro)]

mod auth;
mod build;

#[macro_use]
extern crate rocket;











use rocket::{Data, State};
use anyhow::{anyhow, Result};

use crate::auth::{ApiKey, AuthSystem, TestAuthSystem};
use crate::build::{BuildSystem, FsBuildSystem, ProjectConfig};

#[post("/deploy", data = "<crate_file>")]
fn deploy(state: State<ApiState>, crate_file: Data, api_key: ApiKey) -> Result<String> {
    // Ideally this would be done with Rocket's fairing system, but they
    // don't support state
    if !state.auth_system.authorize(&api_key)? {
        return Err(anyhow!("API key is unauthorized"));
    }

    let project = ProjectConfig {
        name: "some_project".to_string()
    };

    let _build = state.build_system.build(crate_file, &api_key, &project);

    // load so file somehow
    Ok("Done!".to_string())
}

struct ApiState {
    build_system: Box<dyn BuildSystem>,
    auth_system: Box<dyn AuthSystem>
}

fn main() {
    let state = ApiState {
        build_system: Box::new(FsBuildSystem),
        auth_system: Box::new(TestAuthSystem)
    };

    rocket::ignite()
        .manage(state)
        .mount("/", routes![deploy]).launch();
}
