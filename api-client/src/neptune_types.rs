use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub platform_spec: Spec,
    pub ai_lint_report: AiLintReport,
    pub start_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spec {
    pub kind: WorkloadKind,
    pub name: String,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    pub kind: ResourceKind,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkloadKind {
    #[serde(rename = "Backend")]
    Backend,
    #[serde(rename = "ETLJob")]
    ETLJob,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceKind {
    #[serde(rename = "Database")]
    Database,
    #[serde(rename = "ObjectStorageBucket")]
    ObjectStorageBucket,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiLintReport {
    pub compatible: bool,
    #[serde(rename = "generatedAt")]
    pub generated_at: DateTime<Utc>,
    #[serde(default)]
    pub errors: Vec<AiLintFinding>,
    #[serde(default)]
    pub warnings: Vec<AiLintFinding>,
    #[serde(default)]
    pub suppressed: Vec<AiLintFinding>,
    #[serde(default)]
    pub summary: AiLintSummary,
    #[serde(default)]
    pub config: AiLintConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiLintSummary {
    pub errors: u32,
    pub warnings: u32,
    pub suppressed: u32,
    pub blocking: bool,
    #[serde(rename = "blockingReason")]
    pub blocking_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AiLintConfig {
    #[serde(rename = "blockOnWarnings", default)]
    pub block_on_warnings: bool,
    #[serde(rename = "suppressedCodes", default)]
    pub suppressed_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiLintFinding {
    pub category: AiLintCategory,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
    pub details: Option<Value>,
    pub severity: AiLintSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiLintCategory {
    #[serde(rename = "architecture")]
    Architecture,
    #[serde(rename = "resource_support")]
    ResourceSupport,
    #[serde(rename = "workload_support")]
    WorkloadSupport,
    #[serde(rename = "configuration_invalid")]
    ConfigurationInvalid,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiLintSeverity {
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "warning")]
    Warning,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AiLintResponse {
    Report(AiLintReport),
    AiReport { ai_lint_report: AiLintReport },
}

impl AiLintResponse {
    pub fn into_report(self) -> AiLintReport {
        match self {
            AiLintResponse::Report(report) => report,
            AiLintResponse::AiReport { ai_lint_report } => ai_lint_report,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GenerateRequest {
    pub project_zip: Vec<u8>,
    pub project_name: String,
    /// Optional filename for the uploaded zip (defaults to "proj.zip")
    pub file_name: Option<String>,
}

impl GenerateRequest {
    pub fn into_multipart(self) -> reqwest::Result<Form> {
        let fname = self.file_name.unwrap_or_else(|| "proj.zip".to_string());
        let file_part = Part::bytes(self.project_zip)
            .file_name(fname)
            .mime_str("application/octet-stream")?;
        Ok(Form::new()
            .part("project", file_part)
            .text("project_name", self.project_name))
    }
}

impl From<(Vec<u8>, &str)> for GenerateRequest {
    fn from((project_zip, project_name): (Vec<u8>, &str)) -> Self {
        Self {
            project_zip,
            project_name: project_name.to_string(),
            file_name: None,
        }
    }
}

// (duplicate definitions below intentionally removed)

impl From<Vec<u8>> for AiLintRequest {
    fn from(project_zip: Vec<u8>) -> Self {
        Self {
            project_zip,
            file_name: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AiLintRequest {
    pub project_zip: Vec<u8>,
    /// Optional filename for the uploaded zip (defaults to "proj.zip")
    pub file_name: Option<String>,
}

impl AiLintRequest {
    pub fn into_multipart(self) -> reqwest::Result<Form> {
        let fname = self.file_name.unwrap_or_else(|| "proj.zip".to_string());
        let file_part = Part::bytes(self.project_zip)
            .file_name(fname)
            .mime_str("application/zip")?;
        Ok(Form::new().part("project", file_part))
    }
}
