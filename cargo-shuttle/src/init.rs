use std::fmt;
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


#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Framework {
    Axum,
    Rocket,
    Tide,
    Tower,
}

impl fmt::Display for Framework {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Framework::Axum => write!(f, "axum"),
            Framework::Rocket => write!(f, "rocket"),
            Framework::Tide => write!(f, "tide"),
            Framework::Tower => write!(f, "tower"),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Feature {
    Axum,
    Rocket,
    Tide,
    Tower,
}

impl From<Framework> for Feature {
    fn from(framework: Framework) -> Feature {
        match framework {
            Framework::Axum => Feature::Axum,
            Framework::Rocket => Feature::Rocket,
            Framework::Tide => Feature::Tide,
            Framework::Tower => Feature::Tower,
        }
    }
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Feature::Axum => write!(f, "web-axum"),
            Feature::Rocket => write!(f, "web-rocket"),
            Feature::Tide => write!(f, "web-tide"),
            Feature::Tower => write!(f, "web-tower"),
        }
    }
}

pub trait ShuttleInit {
    fn shuttle_init(&mut self) -> Result<()>;
    fn get_boilerplate_code_for_framework(&self) -> &'static str;
}

pub struct ShuttleInitFactory;

impl ShuttleInitFactory {
    pub fn new(path: PathBuf, framework: Framework) -> Box<dyn ShuttleInit> {
        let project_path = path.clone();
        let lib_path = project_path.join("src").join("lib.rs");
        let cargo_toml_path = path.join("Cargo.toml");
        let cargo_doc = read_to_string(cargo_toml_path.clone()).unwrap().parse::<Document>().unwrap();

        match framework {
            Framework::Axum => return Box::new(ShuttleInitAxum {
                cargo_doc,
                cargo_toml_path,
                framework,
                lib_path,
                project_path,
            }),
            Framework::Rocket => return Box::new(ShuttleInitRocket {
                cargo_doc,
                cargo_toml_path,
                framework,
                lib_path,
                project_path,
            }),
            Framework::Tide => return Box::new(ShuttleInitTide {
                cargo_doc,
                cargo_toml_path,
                framework,
                lib_path,
                project_path,
            }),
            Framework::Tower => return Box::new(ShuttleInitTower {
                cargo_doc,
                cargo_toml_path,
                framework,
                lib_path,
                project_path,
            }),
        }
    }
}

pub struct ShuttleInitAxum {
    pub cargo_doc: Document,
    pub cargo_toml_path: PathBuf,
    pub framework: Framework,
    pub lib_path: PathBuf,
    pub project_path: PathBuf,
}

impl ShuttleInitAxum {
    fn set_cargo_dependencies(dependencies: &mut Table, framework: Framework, manifest_path: &PathBuf, url: &Url) {
        // Set "shuttle-service" version to `[dependencies]` table
        set_inline_table_dependency_version(
            "shuttle-service",
             dependencies,
            &manifest_path,
            &url,
        );

        set_key_value_dependency_version(
            &framework.to_string(),
             dependencies,
            &manifest_path,
            &url,
        );

        let feature = Feature::from(framework).to_string();
        set_inline_table_dependency_features(
            "shuttle-service",
             dependencies,
            vec![feature],
        );
        set_key_value_dependency_version(
            "sync_wrapper",
             dependencies,
            &manifest_path,
            &url,
        );
    }
}

impl ShuttleInit for ShuttleInitAxum {
    fn shuttle_init(&mut self) -> Result<()> {
        // Fetch the latest shuttle-service version from crates.io
        let manifest_path = find(Some(&self.project_path)).unwrap();
        let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");

        let dependencies = self.cargo_doc["dependencies"].as_table_mut().unwrap();

        // Set "shuttle-service" version to `[dependencies]` table
        ShuttleInitAxum::set_cargo_dependencies(dependencies, self.framework.clone(), &manifest_path, &url);

        // Truncate Cargo.toml and write the updated `Document` to it
        let mut cargo_toml = File::create(self.cargo_toml_path.clone())?;
        cargo_toml.write_all(self.cargo_doc.to_string().as_bytes())?;

        // Write boilerplate to `src/lib.rs` file
        let boilerplate = self.get_boilerplate_code_for_framework();
        write_lib_file(boilerplate, &self.lib_path)?;

        Ok(())
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

pub struct ShuttleInitRocket {
    pub cargo_doc: Document,
    pub cargo_toml_path: PathBuf,
    pub framework: Framework,
    pub lib_path: PathBuf,
    pub project_path: PathBuf,
}

impl ShuttleInitRocket {
    fn set_cargo_dependencies(dependencies: &mut Table, framework: Framework, manifest_path: &PathBuf, url: &Url) {
        set_inline_table_dependency_version(
            "shuttle-service",
             dependencies,
            &manifest_path,
            &url,
        );

        set_key_value_dependency_version(
            &framework.to_string(),
            dependencies,
            &manifest_path,
            &url,
        );

        let feature = Feature::from(framework).to_string();
        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec![feature],
        );
    }

}

impl ShuttleInit for ShuttleInitRocket {
    fn shuttle_init(&mut self) -> Result<()> {
        // Fetch the latest shuttle-service version from crates.io
        let manifest_path = find(Some(&self.project_path)).unwrap();
        let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");

        let dependencies = self.cargo_doc["dependencies"].as_table_mut().unwrap();

        // Set "shuttle-service" version to `[dependencies]` table
        ShuttleInitRocket::set_cargo_dependencies(dependencies, self.framework.clone(), &manifest_path, &url);
        
        // Truncate Cargo.toml and write the updated `Document` to it
        let mut cargo_toml = File::create(self.cargo_toml_path.clone())?;
        cargo_toml.write_all(self.cargo_doc.to_string().as_bytes())?;

        // Write boilerplate to `src/lib.rs` file
        let boilerplate = self.get_boilerplate_code_for_framework();
        write_lib_file(boilerplate, &self.lib_path)?;

        Ok(())

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

pub struct ShuttleInitTide {
    pub cargo_doc: Document,
    pub cargo_toml_path: PathBuf,
    pub framework: Framework,
    pub lib_path: PathBuf,
    pub project_path: PathBuf,
}

impl ShuttleInitTide {
    fn set_cargo_dependencies(dependencies: &mut Table, framework: Framework, manifest_path: &PathBuf, url: &Url) {
        set_key_value_dependency_version(
            &framework.to_string(),
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

        let feature = Feature::from(framework).to_string();
        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec![feature],
        );
    }
}

impl ShuttleInit for ShuttleInitTide {
    fn shuttle_init(&mut self) -> Result<()> {
        // Fetch the latest shuttle-service version from crates.io
        let manifest_path = find(Some(&self.project_path)).unwrap();
        let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");

        let dependencies = self.cargo_doc["dependencies"].as_table_mut().unwrap();

        // Set "shuttle-service" version to `[dependencies]` table
        ShuttleInitTide::set_cargo_dependencies(dependencies, self.framework.clone(), &manifest_path, &url);
        
        // Truncate Cargo.toml and write the updated `Document` to it
        let mut cargo_toml = File::create(self.cargo_toml_path.clone())?;
        cargo_toml.write_all(self.cargo_doc.to_string().as_bytes())?;

        // Write boilerplate to `src/lib.rs` file
        let boilerplate = self.get_boilerplate_code_for_framework();
        write_lib_file(boilerplate, &self.lib_path)?;

        Ok(())

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

pub struct ShuttleInitTower {
    pub cargo_doc: Document,
    pub cargo_toml_path: PathBuf,
    pub framework: Framework,
    pub lib_path: PathBuf,
    pub project_path: PathBuf,
}

impl ShuttleInitTower {
    fn set_cargo_dependencies(dependencies: &mut Table, framework: Framework, manifest_path: &PathBuf, url: &Url) {
        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_version(
            &framework.to_string(),
            dependencies,
            &manifest_path,
            &url,
        );

        set_inline_table_dependency_features(
            &framework.to_string(),
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

        let feature = Feature::from(framework).to_string();
        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec![feature],
        );
    }
}

impl ShuttleInit for ShuttleInitTower {
    fn shuttle_init(&mut self) -> Result<()> {
        // Fetch the latest shuttle-service version from crates.io
        let manifest_path = find(Some(&self.project_path)).unwrap();
        let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");

        let dependencies = self.cargo_doc["dependencies"].as_table_mut().unwrap();

        // Set "shuttle-service" version to `[dependencies]` table
        ShuttleInitTower::set_cargo_dependencies(dependencies, self.framework.clone(), &manifest_path, &url);
        
        // Truncate Cargo.toml and write the updated `Document` to it
        let mut cargo_toml = File::create(self.cargo_toml_path.clone())?;
        cargo_toml.write_all(self.cargo_doc.to_string().as_bytes())?;

        // Write boilerplate to `src/lib.rs` file
        let boilerplate = self.get_boilerplate_code_for_framework();
        write_lib_file(boilerplate, &self.lib_path)?;

        Ok(())
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

pub fn get_framework(init_args: &InitArgs) -> Option<Framework> {
    if init_args.axum {
        return Some(Framework::Axum);
    }

    if init_args.rocket {
        return Some(Framework::Rocket);
    }

    if init_args.tide {
        return Some(Framework::Tide);
    }

    if init_args.tower {
        return Some(Framework::Tower);
    }

    None
}

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

fn write_lib_file(boilerplate: &'static str, lib_path: &PathBuf) -> Result<()> {
    let mut lib_file = File::create(lib_path.clone())?;
    lib_file.write_all(boilerplate.as_bytes())?;

    Ok(())
}

