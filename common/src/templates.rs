use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Schema used in `examples/templates.toml` and services that parse it
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplatesSchema {
    /// Version of this schema
    pub version: u32,
    /// Mapping of tag names to logo URLs
    pub logos: HashMap<String, String>,
    /// Very basic templates, typically Hello World
    pub starters: HashMap<String, TemplateDefinition>,
    /// Non-starter templates
    pub templates: HashMap<String, TemplateDefinition>,
    /// Examples not meant to be templates
    pub examples: HashMap<String, TemplateDefinition>,
    /// Examples with attached tutorials
    pub tutorials: HashMap<String, TemplateDefinition>,
    /// Templates made by community members
    pub community_templates: HashMap<String, TemplateDefinition>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TemplateDefinition {
    /// Title of the template
    pub title: String,
    /// A short description of the template
    pub description: String,
    /// Path relative to the repo root
    pub path: Option<String>,
    /// List of areas where this template is useful. Examples: "Web app", "Discord bot", "Monitoring", "Automation", "Utility"
    pub use_cases: Vec<String>,
    /// List of keywords that describe the template. Examples: "axum", "serenity", "typescript", "saas", "fullstack", "database"
    pub tags: Vec<String>,
    /// URL to a live instance of the template (if relevant)
    pub live_demo: Option<String>,

    /// If this template is available in the `shuttle init --template` short-hand options, add that name here
    pub template: Option<String>,

    ////// Fields for community templates
    /// GitHub username of the author of the community template
    pub author: Option<String>,
    /// URL to the repo of the community template
    pub repo: Option<String>,
}
