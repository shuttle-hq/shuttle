use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::args::InitArgs;
use anyhow::Result;
use cargo::ops::NewOptions;
use cargo_edit::{find, get_latest_dependency, registry_url};
use indoc::indoc;
use toml_edit::{value, Array, Document, Item, Table};
use url::Url;

pub trait ShuttleInit {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    );
    fn get_boilerplate_code_for_framework(&self) -> &'static str;
}

pub struct ShuttleInitAxum;

impl ShuttleInit for ShuttleInitAxum {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "axum",
            dependencies,
            manifest_path,
            url,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["web-axum".to_string()],
        );
        set_key_value_dependency_version(
            "sync_wrapper",
            dependencies,
            manifest_path,
            url,
            get_dependency_version_fn,
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
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "rocket",
            dependencies,
            manifest_path,
            url,
            get_dependency_version_fn,
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
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["web-tide".to_string()],
        );

        set_key_value_dependency_version(
            "tide",
            dependencies,
            manifest_path,
            url,
            get_dependency_version_fn,
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
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["web-tower".to_string()],
        );

        set_inline_table_dependency_version(
            "tower",
            dependencies,
            manifest_path,
            url,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_features("tower", dependencies, vec!["full".to_string()]);

        set_inline_table_dependency_version(
            "hyper",
            dependencies,
            manifest_path,
            url,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_features("hyper", dependencies, vec!["full".to_string()]);
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

pub struct ShuttleInitNoOp;
impl ShuttleInit for ShuttleInitNoOp {
    fn set_cargo_dependencies(
        &self,
        _dependencies: &mut Table,
        _manifest_path: &Path,
        _url: &Url,
        _get_dependency_version_fn: GetDependencyVersionFn,
    ) {
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        ""
    }
}

/// Returns a framework-specific struct that implements the trait `ShuttleInit`
/// for writing framework-specific dependencies to `Cargo.toml` and generating
/// boilerplate code in `src/lib.rs`.
pub fn get_framework(init_args: &InitArgs) -> Box<dyn ShuttleInit> {
    if init_args.axum {
        return Box::new(ShuttleInitAxum);
    }

    if init_args.rocket {
        return Box::new(ShuttleInitRocket);
    }

    if init_args.tide {
        return Box::new(ShuttleInitTide);
    }

    if init_args.tower {
        return Box::new(ShuttleInitTower);
    }

    Box::new(ShuttleInitNoOp)
}

/// Interoprates with `cargo` crate and calls `cargo init --libs [path]`.
pub fn cargo_init(path: PathBuf) -> Result<()> {
    let opts = NewOptions::new(None, false, true, path, None, None, None)?;
    let cargo_config = cargo::util::config::Config::default()?;
    let init_result = cargo::ops::init(&opts, &cargo_config)?;

    // Mimic `cargo init` behavior and log status or error to shell
    cargo_config
        .shell()
        .status("Created", format!("{} (shuttle) package", init_result))?;

    Ok(())
}

/// Performs shuttle init on the existing files generated by `cargo init --libs [path]`.
pub fn cargo_shuttle_init(path: PathBuf, framework: Box<dyn ShuttleInit>) -> Result<()> {
    let cargo_toml_path = path.join("Cargo.toml");
    let mut cargo_doc = read_to_string(cargo_toml_path.clone())
        .unwrap()
        .parse::<Document>()
        .unwrap();

    // Remove empty dependencies table to re-insert after the lib table is inserted
    cargo_doc.remove("dependencies");

    // Create an empty `[lib]` table
    cargo_doc["lib"] = Item::Table(Table::new());

    // Create `[dependencies]` table
    let mut dependencies = Table::new();

    // Set "shuttle-service" version to `[dependencies]` table
    let manifest_path = find(Some(path.as_path())).unwrap();
    let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");
    set_inline_table_dependency_version(
        "shuttle-service",
        &mut dependencies,
        &manifest_path,
        &url,
        get_latest_dependency_version,
    );

    // Set framework-specific dependencies to the `dependencies` table
    framework.set_cargo_dependencies(
        &mut dependencies,
        &manifest_path,
        &url,
        get_latest_dependency_version,
    );

    // Truncate Cargo.toml and write the updated `Document` to it
    let mut cargo_toml = File::create(cargo_toml_path)?;
    cargo_doc["dependencies"] = Item::Table(dependencies);
    cargo_toml.write_all(cargo_doc.to_string().as_bytes())?;

    // Write boilerplate to `src/lib.rs` file
    let lib_path = path.join("src").join("lib.rs");
    let boilerplate = framework.get_boilerplate_code_for_framework();
    if !boilerplate.is_empty() {
        write_lib_file(boilerplate, &lib_path)?;
    }

    Ok(())
}

/// Sets dependency version for a key-value pair:
/// `crate_name = "version"`
fn set_key_value_dependency_version(
    crate_name: &str,
    dependencies: &mut Table,
    manifest_path: &Path,
    url: &Url,
    get_dependency_version_fn: GetDependencyVersionFn,
) {
    let dependency_version = get_dependency_version_fn(crate_name, manifest_path, url);
    dependencies[crate_name] = value(dependency_version);
}

/// Sets dependency version for an inline table:
/// `crate_name = { version = "version" }`
fn set_inline_table_dependency_version(
    crate_name: &str,
    dependencies: &mut Table,
    manifest_path: &Path,
    url: &Url,
    get_dependency_version_fn: GetDependencyVersionFn,
) {
    let dependency_version = get_dependency_version_fn(crate_name, manifest_path, url);
    dependencies[crate_name]["version"] = value(dependency_version);
}

/// Sets dependency features for an inline table:
/// `crate_name = { features = ["some-feature"] }`
fn set_inline_table_dependency_features(
    crate_name: &str,
    dependencies: &mut Table,
    features: Vec<String>,
) {
    let features = Array::from_iter(features);
    dependencies[crate_name]["features"] = value(features);
}

/// Abstract type for `get_latest_dependency_version` function.
type GetDependencyVersionFn = fn(&str, &Path, &Url) -> String;

/// Gets the latest version for a dependency of `crate_name`.
/// This is a wrapper function for `cargo_edit::get_latest_dependency` function.
fn get_latest_dependency_version(crate_name: &str, manifest_path: &Path, url: &Url) -> String {
    let latest_version = get_latest_dependency(crate_name, false, manifest_path, Some(url))
        .unwrap_or_else(|_| panic!("Could not query the latest version of {}", crate_name));
    let latest_version = latest_version
        .version()
        .expect("No latest shuttle-service version available");

    latest_version.to_string()
}

/// Writes `boilerplate` code to the specified `lib.rs` file path.
pub fn write_lib_file(boilerplate: &'static str, lib_path: &Path) -> Result<()> {
    let mut lib_file = File::create(lib_path)?;
    lib_file.write_all(boilerplate.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod shuttle_init_tests {
    use super::*;

    fn init_args_factory(framework: &str) -> InitArgs {
        let mut init_args = InitArgs {
            axum: false,
            rocket: false,
            tide: false,
            tower: false,
            path: PathBuf::new(),
        };

        match framework {
            "axum" => init_args.axum = true,
            "rocket" => init_args.rocket = true,
            "tide" => init_args.tide = true,
            "tower" => init_args.tower = true,
            _ => unreachable!(),
        }

        init_args
    }

    fn cargo_toml_factory() -> Document {
        indoc! {r#"
            [dependencies]
        "#}
        .parse::<Document>()
        .unwrap()
    }

    fn mock_get_latest_dependency_version(
        _crate_name: &str,
        _manifest_path: &Path,
        _url: &Url,
    ) -> String {
        "1.0".to_string()
    }

    #[test]
    fn test_get_framework_via_get_boilerplate_code() {
        let frameworks = vec!["axum", "rocket", "tide", "tower"];
        let framework_inits: Vec<Box<dyn ShuttleInit>> = vec![
            Box::new(ShuttleInitAxum),
            Box::new(ShuttleInitRocket),
            Box::new(ShuttleInitTide),
            Box::new(ShuttleInitTower),
        ];

        for (framework, expected_framework_init) in frameworks.into_iter().zip(framework_inits) {
            let framework_init = get_framework(&init_args_factory(framework));
            assert_eq!(
                framework_init.get_boilerplate_code_for_framework(),
                expected_framework_init.get_boilerplate_code_for_framework(),
            );
        }
    }

    #[test]
    fn test_set_inline_table_dependency_features() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();

        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["test-feature".to_string()],
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { features = ["test-feature"] }
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_inline_table_dependency_version() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { version = "1.0" }
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_key_value_dependency_version() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_axum() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        ShuttleInitAxum.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { version = "1.0", features = ["web-axum"] }
            axum = "1.0"
            sync_wrapper = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_rocket() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        ShuttleInitRocket.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { version = "1.0", features = ["web-rocket"] }
            rocket = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_tide() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        ShuttleInitTide.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { version = "1.0", features = ["web-tide"] }
            tide = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_tower() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_inline_table_dependency_version(
            "shuttle-service",
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        ShuttleInitTower.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { version = "1.0", features = ["web-tower"] }
            tower = { version = "1.0", features = ["full"] }
            hyper = { version = "1.0", features = ["full"] }
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }
}
