use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub platform_spec: Spec,
    pub compatibility_report: CompatibilityReport,
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
pub struct CompatibilityReport {
    pub compatible: bool,
    #[serde(rename = "generatedAt")]
    pub generated_at: DateTime<Utc>,
    pub errors: Vec<CompatError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatError {
    pub category: ErrorCategory,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCategory {
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

impl From<Vec<u8>> for CheckCompatibilityRequest {
    fn from(project_zip: Vec<u8>) -> Self {
        Self {
            project_zip,
            file_name: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CheckCompatibilityRequest {
    pub project_zip: Vec<u8>,
    /// Optional filename for the uploaded zip (defaults to "proj.zip")
    pub file_name: Option<String>,
}

impl CheckCompatibilityRequest {
    pub fn into_multipart(self) -> reqwest::Result<Form> {
        let fname = self.file_name.unwrap_or_else(|| "proj.zip".to_string());
        let file_part = Part::bytes(self.project_zip)
            .file_name(fname)
            .mime_str("application/zip")?;
        Ok(Form::new().part("project", file_part))
    }
}
