use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use cargo::ops::NewOptions;
use cargo_edit::{find, get_latest_dependency, registry_url};
use crate::args::InitArgs;
use indoc::indoc;
use toml_edit::{value, Array, Document, Item, Table};
use url::Url;

pub trait ShuttleInit {
    fn set_cargo_dependencies(&self, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url);
    fn get_boilerplate_code_for_framework(&self) -> &'static str;
}

pub struct ShuttleInitAxum;

impl ShuttleInit for ShuttleInitAxum {
    fn set_cargo_dependencies(&self, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url) {
        // Set "shuttle-service" version to `[dependencies]` table
        set_inline_table_dependency_version(
            "shuttle-service",
             dependencies,
            &manifest_path,
            &url,
        );

        set_key_value_dependency_version(
            "axum",
             dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_features(
            "shuttle-service",
             dependencies,
            vec!["web-axum".to_string()],
        );
        set_key_value_dependency_version(
            "sync_wrapper",
             dependencies,
            &manifest_path,
            &url,
        );
    }
    
    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use axum::{routing::get, Router};
        use sync_wrapper::SyncWrapper;

        async fn hello_world() -> &'static str {
            "Hello, world!"
        }

        #[shuttle_service::main]
        async fn axum() -> shuttle_service::ShuttleAxum {
            let router = Router::new().route("/hello", get(hello_world));
            let sync_wrapper = SyncWrapper::new(router);

            Ok(sync_wrapper){}
        }"#}
    }
}

pub struct ShuttleInitRocket;

impl ShuttleInit for ShuttleInitRocket {
    fn set_cargo_dependencies(&self, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url) {
        set_inline_table_dependency_version(
            "shuttle-service",
             dependencies,
            &manifest_path,
            &url,
        );

        set_key_value_dependency_version(
            "rocket",
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["web-rocket".to_string()],
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        #[macro_use]
        extern crate rocket;

        #[get("/")]
        fn index() -> &'static str {
            "Hello, world!"
        }

        #[shuttle_service::main]
        async fn rocket() -> shuttle_service::ShuttleRocket {
            let rocket = rocket::build().mount("/hello", routes![index]);

            Ok(rocket)
        }"#}
    }
}

pub struct ShuttleInitTide;

impl ShuttleInit for ShuttleInitTide {
    fn set_cargo_dependencies(&self, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url) {
        set_key_value_dependency_version(
            "tide",
            dependencies,
            &manifest_path,
            &url,
        );

        // Set "shuttle-service" version to `[dependencies]` table
        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["web-tide".to_string()],
        );
    }
    
    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        #[shuttle_service::main]
        async fn tide() -> shuttle_service::ShuttleTide<()> {
            let mut app = tide::new();
            app.with(tide::log::LogMiddleware::new());

            app.at("/hello").get(|_| async { Ok("Hello, world!") });

            Ok(app)
        }"#}
    }
}

pub struct ShuttleInitTower;

impl ShuttleInit for ShuttleInitTower {
    fn set_cargo_dependencies(&self, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url) {
        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_version(
            "tower",
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_features(
            "tower",
            dependencies,
            vec!["full".to_string()],
        );

        set_inline_table_dependency_version(
            "hyper",
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_features(
            "hyper",
            dependencies,
            vec!["full".to_string()],
        );

        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["web-tower".to_string()],
        );
    }
    
    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use std::convert::Infallible;
        use std::future::Future;
        use std::pin::Pin;
        use std::task::{Context, Poll};

        #[derive(Clone)]
        struct HelloWorld;

        impl tower::Service<hyper::Request<hyper::Body>> for HelloWorld {
            type Response = hyper::Response<hyper::Body>;
            type Error = Infallible;
            type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, _req: hyper::Request<hyper::Body>) -> Self::Future {
                let body = hyper::Body::from("Hello, world!");
                let resp = hyper::Response::builder()
                    .status(200)
                    .body(body)
                    .expect("Unable to create the `hyper::Response` object");

                let fut = async { Ok(resp) };

                Box::pin(fut)
            }
        }

        #[shuttle_service::main]
        async fn tower() -> Result<HelloWorld, shuttle_service::Error> {
            Ok(HelloWorld)
        }"#}
    }
}

/// Returns a framework-specific struct that implements the trait `ShuttleInit`
/// for writing framework-specific dependencies to `Cargo.toml` and generating 
/// boilerplate code in `src/lib.rs`.
pub fn get_framework(init_args: &InitArgs) -> Option<Box<dyn ShuttleInit>> {
    if init_args.axum {
        return Some(
            Box::new(ShuttleInitAxum)
        );
    }

    if init_args.rocket {
        return Some(
            Box::new(ShuttleInitRocket)
        );
    }

    if init_args.tide {
        return Some(
            Box::new(ShuttleInitTide)
        );
    }

    if init_args.tower {
        return Some(
            Box::new(ShuttleInitTower)
        );
    }

    None
}

/// Interoprates with `cargo` crate and calls `cargo init --libs [path]`.
pub fn cargo_init(path: PathBuf) -> Result<()> {
    let opts = NewOptions::new(None, false, true, path, None, None, None)?;
    let cargo_config = cargo::util::config::Config::default()?;
    let init_result = cargo::ops::init(&opts, &cargo_config)?;

    // Mimick `cargo init` behavior and log status or error to shell
    cargo_config
        .shell()
        .status("Created", format!("{} (shuttle) package", init_result))?;

    Ok(())
}

/// Processes `Cargo.toml` after calling `cargo_init` function to re-order `lib` and `dependencies`
/// tables as well as inserting `shuttle-service` dependency to the `dependencies` table.
pub fn process_cargo_init(path: PathBuf) -> Result<()> {
    let cargo_toml_path = path.join("Cargo.toml");
    let mut cargo_doc = read_to_string(cargo_toml_path.clone()).unwrap().parse::<Document>().unwrap();
    
    // Remove empty dependencies table to re-insert after the lib table is inserted
    cargo_doc.remove("dependencies");

    // Create an empty `[lib]` table
    cargo_doc["lib"] = Item::Table(Table::new());

    // Fetch the latest shuttle-service version from crates.io
    let manifest_path = find(Some(path.as_path())).unwrap();
    let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");

    // Create `[dependencies]` table
    let mut dependencies = Table::new();

    // Set "shuttle-service" version to `[dependencies]` table
    set_inline_table_dependency_version(
        "shuttle-service",
        &mut dependencies,
        &manifest_path,
        &url,
    );

    cargo_doc["dependencies"] = Item::Table(dependencies);

    // Truncate Cargo.toml and write the updated `Document` to it
    let mut cargo_toml = File::create(cargo_toml_path.clone())?;
    cargo_toml.write_all(cargo_doc.to_string().as_bytes())?;

    Ok(())
}

/// Generates framework-specific dependencies to `Cargo.toml` and boilerplate code for `src/lib.rs`.
pub fn framework_init(project_path: &PathBuf, framework: Box<dyn ShuttleInit>) -> Result<()> {
    let project_path = project_path.clone();
    let lib_path = project_path.join("src").join("lib.rs");
    let cargo_toml_path = project_path.join("Cargo.toml");
    let mut cargo_doc = read_to_string(cargo_toml_path.clone()).unwrap().parse::<Document>().unwrap();
    
    let manifest_path = find(Some(&project_path)).unwrap();
    let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");
    let dependencies = cargo_doc["dependencies"].as_table_mut().unwrap();
    
    framework.set_cargo_dependencies(dependencies, &manifest_path, &url);
    
    let mut cargo_toml = File::create(cargo_toml_path.clone())?;
    cargo_toml.write_all(cargo_doc.to_string().as_bytes())?;

    // Write boilerplate to `src/lib.rs` file
    let boilerplate = framework.get_boilerplate_code_for_framework();
    write_lib_file(boilerplate, &lib_path)?;

    Ok(())
}

fn set_key_value_dependency_version(crate_name: &str, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url) {
    let dependency_version = get_latest_dependency_version(crate_name, &manifest_path, &url);
    dependencies[crate_name] = value(dependency_version);
}

fn set_inline_table_dependency_version(crate_name: &str, dependencies: &mut Table, manifest_path: &PathBuf, url: &Url) {
    let dependency_version = get_latest_dependency_version(crate_name, &manifest_path, &url);
    dependencies[crate_name]["version"] = value(dependency_version);
}

fn set_inline_table_dependency_features(crate_name: &str, dependencies: &mut Table, features: Vec<String>) {
    let features = Array::from_iter(features);
    dependencies[crate_name]["features"] = value(features);
}

fn get_latest_dependency_version(crate_name: &str, manifest_path: &PathBuf, url: &Url) -> String {
    let latest_version =
        get_latest_dependency(crate_name, false, &manifest_path, Some(&url))
            .expect(&format!("Could not query the latest version of {}", crate_name));
    let latest_version = latest_version
        .version()
        .expect("No latest shuttle-service version available");

    latest_version.to_string()
}

pub fn write_lib_file(boilerplate: &'static str, lib_path: &PathBuf) -> Result<()> {
    let mut lib_file = File::create(lib_path.clone())?;
    lib_file.write_all(boilerplate.as_bytes())?;

    Ok(())
}

