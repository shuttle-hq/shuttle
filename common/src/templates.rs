use serde::{Deserialize, Serialize};

/// Schema used in `examples/templates.toml` and services that parses it
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TemplateDefinition {
    /// Title of the template
    title: String,
    /// A short description of the template
    description: Option<String>,
    /// Path relative to the repo root
    path: Option<String>,
    /// "starter" OR "template" (default) OR "tutorial"
    #[serde(default)]
    r#type: TemplateType,
    /// List of areas where this template is useful. Examples: "Web app", "Discord bot", "Monitoring", "Automation", "Utility"
    use_cases: Vec<String>,
    /// List of keywords that describe the template. Examples: "axum", "serenity", "typescript", "saas", "fullstack", "database"
    tags: Vec<String>,
    /// URL to a live instance of the template (if relevant)
    live_demo: Option<String>,

    /// If this template is available in the `cargo shuttle init --template` short-hand options, add that name here
    template: Option<String>,

    /// Set this to true if this is a community template outside of the shuttle-examples repo
    community: Option<bool>,
    /// GitHub username of the author of the community template
    author: Option<String>,
    /// URL to the repo of the community template
    repo: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemplateType {
    Starter,
    #[default]
    Template,
    Tutorial,
}
