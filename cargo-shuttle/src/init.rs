use std::path::PathBuf;

use anyhow::{bail, Result};
use shuttle_common::project::ProjectName;

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
    fn get_repo_url(&self) -> &'static str;
    fn get_sub_path(&self) -> Option<&'static str>;
}

pub struct ShuttleInitActixWeb;

impl ShuttleInit for ShuttleInitActixWeb {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("actix-web/hello-world")
    }
}

pub struct ShuttleInitAxum;

impl ShuttleInit for ShuttleInitAxum {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("axum/hello-world")
    }
}

pub struct ShuttleInitRocket;

impl ShuttleInit for ShuttleInitRocket {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("shuttle/hello-world")
    }
}

pub struct ShuttleInitTide;

impl ShuttleInit for ShuttleInitTide {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("tide/hello-world")
    }
}

pub struct ShuttleInitPoem;

impl ShuttleInit for ShuttleInitPoem {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("poem/hello-world")
    }
}

pub struct ShuttleInitSalvo;

impl ShuttleInit for ShuttleInitSalvo {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("salvo/hello-world")
    }
}

pub struct ShuttleInitSerenity;

impl ShuttleInit for ShuttleInitSerenity {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("serenity/hello-world")
    }
}

pub struct ShuttleInitPoise;

impl ShuttleInit for ShuttleInitPoise {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("poise/hello-world")
    }
}

pub struct ShuttleInitTower;

impl ShuttleInit for ShuttleInitTower {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("tower/hello-world")
    }
}

pub struct ShuttleInitWarp;

impl ShuttleInit for ShuttleInitWarp {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("warp/hello-world")
    }
}

pub struct ShuttleInitThruster;

impl ShuttleInit for ShuttleInitThruster {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        Some("thruster/hello-world")
    }
}

pub struct ShuttleInitNoOp;
impl ShuttleInit for ShuttleInitNoOp {
    fn get_repo_url(&self) -> &'static str {
        "http://github.com/shuttle-hq/shuttle-examples.git"
    }

    fn get_sub_path(&self) -> Option<&'static str> {
        todo!()
    }
}

pub fn cargo_generate(path: PathBuf, name: ProjectName, framework: Template) -> Result<()> {
    let config = framework.init_config();

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("generate")
        .arg("--init")
        .arg("--git")
        .arg(&config.get_repo_url())
        .arg(&config.get_sub_path().unwrap_or("."))
        .arg("--name")
        .arg(name.as_str())
        .arg("--destination")
        .arg(path.as_os_str());
    println!(r#"    Creating project "{name}" in {path:?}"#);
    
    let output = cmd.output().expect("Failed to initialize with cargo generate.");
    let stderr = String::from_utf8(output.stderr).unwrap();
    if !output.status.success() {
        bail!("cargo generate failed:\n{}", stderr)
    }
    print!("{}", stderr);

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
