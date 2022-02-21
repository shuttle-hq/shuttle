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

    let build = state.build_system.build(crate_file, &api_key, &project)?;

    let service = unsafe {
        let so = libloading::Library::new("libtemp.so")?;
        // TODO: `fn() -> u64` is of course temporary - will instead return `Box<dyn Service>`
        let entrypoint: libloading::Symbol<unsafe extern fn() -> u64> = so.get(b"entrypoint\0")?;
        entrypoint()
    };

    // ...

    Ok(service.to_string())
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
