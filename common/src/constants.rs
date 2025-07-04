//! Shared constants used across Shuttle crates

// URLs
pub const SHUTTLE_API_URL: &str = "https://api.shuttle.dev";
pub const SHUTTLE_CONSOLE_URL: &str = "https://console.shuttle.dev";

pub fn other_env_api_url(env: &str) -> String {
    format!("https://api.{env}.shuttle.dev")
}

pub const SHUTTLE_INSTALL_DOCS_URL: &str = "https://docs.shuttle.dev/getting-started/installation";

pub const SHUTTLE_GH_REPO_URL: &str = "https://github.com/shuttle-hq/shuttle";
pub const SHUTTLE_GH_ISSUE_URL: &str = "https://github.com/shuttle-hq/shuttle/issues/new/choose";
pub const EXAMPLES_REPO: &str = "https://github.com/shuttle-hq/shuttle-examples";
pub const EXAMPLES_README: &str =
    "https://github.com/shuttle-hq/shuttle-examples#how-to-clone-run-and-deploy-an-example";
pub const EXAMPLES_TEMPLATES_TOML: &str =
    "https://raw.githubusercontent.com/shuttle-hq/shuttle-examples/main/templates.toml";

/// Current version field in `examples/templates.toml`
pub const TEMPLATES_SCHEMA_VERSION: u32 = 1;

pub mod headers {
    use http::HeaderName;

    pub static X_CARGO_SHUTTLE_VERSION: HeaderName =
        HeaderName::from_static("x-cargo-shuttle-version");
}
