use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use cargo_edit::{find, get_latest_dependency, registry_url};
use shuttle_common::project::ProjectName;
use toml_edit::{value, Array, Document, Table};
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display, strum::EnumIter)]
#[strum(serialize_all = "kebab-case")]
pub enum Template {
    ActixWeb,
    Axum,
    Poise,
    Poem,
    Rocket,
    Salvo,
    Serenity,
    Tide,
    Thruster,
    Tower,
    Warp,
    None,
}

impl Template {
    /// Returns a framework-specific struct that implements the trait `ShuttleInit`
    /// for writing framework-specific dependencies to `Cargo.toml` and generating
    /// boilerplate code in `src/main.rs`.
    pub fn init_config(&self) -> Box<dyn ShuttleInit> {
        use Template::*;
        match self {
            ActixWeb => Box::new(ShuttleInitActixWeb),
            Axum => Box::new(ShuttleInitAxum),
            Rocket => Box::new(ShuttleInitRocket),
            Tide => Box::new(ShuttleInitTide),
            Tower => Box::new(ShuttleInitTower),
            Poem => Box::new(ShuttleInitPoem),
            Salvo => Box::new(ShuttleInitSalvo),
            Serenity => Box::new(ShuttleInitSerenity),
            Poise => Box::new(ShuttleInitPoise),
            Warp => Box::new(ShuttleInitWarp),
            Thruster => Box::new(ShuttleInitThruster),
            None => Box::new(ShuttleInitNoOp),
        }
    }
}

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

pub struct ShuttleInitActixWeb;

impl ShuttleInit for ShuttleInitActixWeb {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "actix-web",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-actix-web",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/actix-web/hello-world/src/main.rs")
    }
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
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-axum",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/axum/hello-world/src/main.rs")
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
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-rocket",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/rocket/hello-world/src/main.rs")
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
        set_key_value_dependency_version(
            "shuttle-tide",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tide",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/tide/hello-world/src/main.rs")
    }
}

pub struct ShuttleInitPoem;

impl ShuttleInit for ShuttleInitPoem {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "poem",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-poem",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/poem/hello-world/src/main.rs")
    }
}

pub struct ShuttleInitSalvo;

impl ShuttleInit for ShuttleInitSalvo {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "salvo",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-salvo",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/salvo/hello-world/src/main.rs")
    }
}

pub struct ShuttleInitSerenity;

impl ShuttleInit for ShuttleInitSerenity {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "anyhow",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_version(
            "serenity",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        dependencies["serenity"]["default-features"] = value(false);

        set_inline_table_dependency_features(
            "serenity",
            dependencies,
            vec![
                "client".into(),
                "gateway".into(),
                "rustls_backend".into(),
                "model".into(),
            ],
        );

        set_key_value_dependency_version(
            "shuttle-secrets",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-serenity",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tracing",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/serenity/hello-world/src/main.rs")
    }
}

pub struct ShuttleInitPoise;

impl ShuttleInit for ShuttleInitPoise {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "anyhow",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "poise",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-poise",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "shuttle-secrets",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tracing",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/poise/hello-world/src/main.rs")
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
        set_inline_table_dependency_version(
            "hyper",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_features("hyper", dependencies, vec!["full".to_string()]);

        set_key_value_dependency_version(
            "shuttle-tower",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_version(
            "tower",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_features("tower", dependencies, vec!["full".to_string()]);
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/tower/hello-world/src/main.rs")
    }
}

pub struct ShuttleInitWarp;

impl ShuttleInit for ShuttleInitWarp {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "shuttle-warp",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_key_value_dependency_version(
            "warp",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/warp/hello-world/src/main.rs")
    }
}

pub struct ShuttleInitThruster;

impl ShuttleInit for ShuttleInitThruster {
    fn set_cargo_dependencies(
        &self,
        dependencies: &mut Table,
        manifest_path: &Path,
        url: &Url,
        get_dependency_version_fn: GetDependencyVersionFn,
    ) {
        set_key_value_dependency_version(
            "shuttle-thruster",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_version(
            "thruster",
            dependencies,
            manifest_path,
            url,
            false,
            get_dependency_version_fn,
        );

        set_inline_table_dependency_features(
            "thruster",
            dependencies,
            vec!["hyper_server".to_string()],
        );

        set_key_value_dependency_version(
            "tokio",
            dependencies,
            manifest_path,
            url,
            true,
            get_dependency_version_fn,
        );
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        include_str!("../../examples/thruster/hello-world/src/main.rs")
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

pub fn cargo_init(path: PathBuf, name: ProjectName) -> Result<()> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("init")
        .arg("--bin")
        .arg("--name")
        .arg(name.as_str())
        .arg(path.as_os_str());
    println!(r#"    Creating project "{name}" in {path:?}"#);
    let output = cmd.output().expect("Failed to initialize with cargo init.");
    let stderr = String::from_utf8(output.stderr).unwrap();
    if !output.status.success() {
        bail!("cargo init failed:\n{}", stderr)
    }
    print!("{}", stderr);

    Ok(())
}

/// Performs shuttle init on the existing files generated by `cargo init [path]`.
pub fn cargo_shuttle_init(path: PathBuf, framework: Template) -> Result<()> {
    println!(r#"     Setting up "{framework}" template"#);
    let cargo_toml_path = path.join("Cargo.toml");
    let mut cargo_doc = read_to_string(cargo_toml_path.clone())
        .unwrap()
        .parse::<Document>()
        .unwrap();

    // Add publish: false to avoid accidental `cargo publish`
    cargo_doc["package"]["publish"] = value(false);

    // Get `[dependencies]` table
    let dependencies = cargo_doc["dependencies"]
        .as_table_mut()
        .expect("manifest to have a dependencies table");

    let manifest_path = find(Some(path.as_path())).unwrap();
    let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");

    let init_config = framework.init_config();

    set_key_value_dependency_version(
        "shuttle-runtime",
        dependencies,
        &manifest_path,
        &url,
        true, // TODO: disallow pre-release when releasing 0.12?
        get_latest_dependency_version,
    );

    // Set framework-specific dependencies to the `dependencies` table
    init_config.set_cargo_dependencies(
        dependencies,
        &manifest_path,
        &url,
        get_latest_dependency_version,
    );

    // Truncate Cargo.toml and write the updated `Document` to it
    let mut cargo_toml = File::create(cargo_toml_path)?;

    cargo_toml.write_all(cargo_doc.to_string().as_bytes())?;

    // Write boilerplate to `src/main.rs` file
    let main_path = path.join("src").join("main.rs");
    let boilerplate = init_config.get_boilerplate_code_for_framework();
    if !boilerplate.is_empty() {
        write_main_file(boilerplate, &main_path)?;
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
    flag_allow_prerelease: bool,
    get_dependency_version_fn: GetDependencyVersionFn,
) {
    let dependency_version =
        get_dependency_version_fn(crate_name, flag_allow_prerelease, manifest_path, url);
    dependencies[crate_name] = value(dependency_version);
}

/// Sets dependency version for an inline table:
/// `crate_name = { version = "version" }`
fn set_inline_table_dependency_version(
    crate_name: &str,
    dependencies: &mut Table,
    manifest_path: &Path,
    url: &Url,
    flag_allow_prerelease: bool,
    get_dependency_version_fn: GetDependencyVersionFn,
) {
    let dependency_version =
        get_dependency_version_fn(crate_name, flag_allow_prerelease, manifest_path, url);
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
type GetDependencyVersionFn = fn(&str, bool, &Path, &Url) -> String;

/// Gets the latest version for a dependency of `crate_name`.
/// This is a wrapper function for `cargo_edit::get_latest_dependency` function.
fn get_latest_dependency_version(
    crate_name: &str,
    flag_allow_prerelease: bool,
    manifest_path: &Path,
    url: &Url,
) -> String {
    let latest_version =
        get_latest_dependency(crate_name, flag_allow_prerelease, manifest_path, Some(url))
            .unwrap_or_else(|_| panic!("Could not query the latest version of {crate_name}"));
    let latest_version = latest_version
        .version()
        .expect("No latest shuttle-service version available");

    latest_version.to_string()
}

/// Writes `boilerplate` code to the specified `main.rs` file path.
pub fn write_main_file(boilerplate: &'static str, main_path: &Path) -> Result<()> {
    let mut main_file = File::create(main_path)?;
    main_file.write_all(boilerplate.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod shuttle_init_tests {
    use super::*;

    fn cargo_toml_factory() -> Document {
        indoc! {r#"
            [dependencies]
        "#}
        .parse::<Document>()
        .unwrap()
    }

    fn mock_get_latest_dependency_version(
        _crate_name: &str,
        _flag_allow_prerelease: bool,
        _manifest_path: &Path,
        _url: &Url,
    ) -> String {
        "1.0".to_string()
    }

    #[test]
    fn test_set_inline_table_dependency_features() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();

        set_inline_table_dependency_features(
            "shuttle-service",
            dependencies,
            vec!["builder".to_string()],
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = { features = ["builder"] }
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
            false,
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
            false,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-service = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }
    #[test]
    fn test_set_cargo_dependencies_actix_web() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitActixWeb.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            actix-web = "1.0"
            shuttle-actix-web = "1.0"
            tokio = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_axum() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
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
            shuttle-runtime = "1.0"
            axum = "1.0"
            shuttle-axum = "1.0"
            tokio = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_rocket() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
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
            shuttle-runtime = "1.0"
            rocket = "1.0"
            shuttle-rocket = "1.0"
            tokio = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_tide() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
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
            shuttle-runtime = "1.0"
            shuttle-tide = "1.0"
            tokio = "1.0"
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

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
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
            shuttle-runtime = "1.0"
            hyper = { version = "1.0", features = ["full"] }
            shuttle-tower = "1.0"
            tokio = "1.0"
            tower = { version = "1.0", features = ["full"] }
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_poem() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitPoem.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            poem = "1.0"
            shuttle-poem = "1.0"
            tokio = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_salvo() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitSalvo.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            salvo = "1.0"
            shuttle-salvo = "1.0"
            tokio = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_serenity() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitSerenity.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            anyhow = "1.0"
            serenity = { version = "1.0", default-features = false, features = ["client", "gateway", "rustls_backend", "model"] }
            shuttle-secrets = "1.0"
            shuttle-serenity = "1.0"
            tokio = "1.0"
            tracing = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_poise() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitPoise.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            anyhow = "1.0"
            poise = "1.0"
            shuttle-poise = "1.0"
            shuttle-secrets = "1.0"
            tokio = "1.0"
            tracing = "1.0"
		"#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_warp() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitWarp.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            shuttle-warp = "1.0"
            tokio = "1.0"
            warp = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    #[test]
    fn test_set_cargo_dependencies_thruster() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://shuttle.rs").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitThruster.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            mock_get_latest_dependency_version,
        );

        let expected = indoc! {r#"
            [dependencies]
            shuttle-runtime = "1.0"
            shuttle-thruster = "1.0"
            thruster = { version = "1.0", features = ["hyper_server"] }
            tokio = "1.0"
        "#};

        assert_eq!(cargo_toml.to_string(), expected);
    }

    // TODO: unignore this test when we publish shuttle-rocket
    #[ignore]
    #[test]
    /// Makes sure that Rocket uses allow_prerelease flag when fetching the latest version
    fn test_get_latest_dependency_version_rocket() {
        let mut cargo_toml = cargo_toml_factory();
        let dependencies = cargo_toml["dependencies"].as_table_mut().unwrap();
        let manifest_path = PathBuf::new();
        let url = Url::parse("https://github.com/rust-lang/crates.io-index").unwrap();

        set_key_value_dependency_version(
            "shuttle-runtime",
            dependencies,
            &manifest_path,
            &url,
            true,
            mock_get_latest_dependency_version,
        );

        ShuttleInitRocket.set_cargo_dependencies(
            dependencies,
            &manifest_path,
            &url,
            get_latest_dependency_version,
        );

        let version = dependencies["rocket"].as_str().unwrap();

        let expected = get_latest_dependency("rocket", true, &manifest_path, Some(&url))
            .expect("Could not query the latest version of rocket")
            .version()
            .expect("no rocket version found")
            .to_string();

        assert_eq!(version, expected);
    }
}
