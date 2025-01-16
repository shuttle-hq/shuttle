use comfy_table::{
    presets::{NOTHING, UTF8_BORDERS_ONLY},
    ContentArrangement, Table,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{secrets::SecretStore, DatabaseInfoBeta};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct ProvisionResourceRequestBeta {
    /// The type of this resource
    pub r#type: ResourceTypeBeta,
    /// The config used when creating this resource.
    /// Use `Self::r#type` to know how to parse this data.
    pub config: Value,
}

/// Helper for deserializing
#[derive(Deserialize)]
#[serde(untagged)] // Try deserializing as a Shuttle resource, fall back to a custom value
pub enum ResourceInputBeta {
    Shuttle(ProvisionResourceRequestBeta),
    Custom(Value),
}

/// The resource state represents the stage of the provisioning process the resource is in.
#[derive(
    Debug, Clone, PartialEq, Eq, strum::Display, strum::EnumString, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
#[typeshare::typeshare]
pub enum ResourceState {
    Authorizing,
    Provisioning,
    Failed,
    Ready,
    Deleting,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[typeshare::typeshare]
pub struct ResourceResponseBeta {
    pub r#type: ResourceTypeBeta,
    pub state: ResourceState,
    /// The config used when creating this resource. Use the `r#type` to know how to parse this data.
    pub config: Value,
    /// The output type for this resource, if state is Ready. Use the `r#type` to know how to parse this data.
    pub output: Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[typeshare::typeshare]
pub struct ResourceListResponseBeta {
    pub resources: Vec<ResourceResponseBeta>,
}

#[derive(
    Clone, Copy, Debug, strum::EnumString, strum::Display, Deserialize, Serialize, Eq, PartialEq,
)]
#[typeshare::typeshare]
// is a flat enum instead of nested enum to allow typeshare
pub enum ResourceTypeBeta {
    #[strum(to_string = "database::shared::postgres")]
    #[serde(rename = "database::shared::postgres")]
    DatabaseSharedPostgres,
    #[strum(to_string = "database::aws_rds::postgres")]
    #[serde(rename = "database::aws_rds::postgres")]
    DatabaseAwsRdsPostgres,
    #[strum(to_string = "database::aws_rds::mysql")]
    #[serde(rename = "database::aws_rds::mysql")]
    DatabaseAwsRdsMySql,
    #[strum(to_string = "database::aws_rds::mariadb")]
    #[serde(rename = "database::aws_rds::mariadb")]
    DatabaseAwsRdsMariaDB,
    /// (Will probably be removed)
    #[strum(to_string = "secrets")]
    #[serde(rename = "secrets")]
    Secrets,
    /// Local provisioner only
    #[strum(to_string = "container")]
    #[serde(rename = "container")]
    Container,
}

pub fn get_resource_tables_beta(
    resources: &[ResourceResponseBeta],
    service_name: &str,
    raw: bool,
    show_secrets: bool,
) -> String {
    if resources.is_empty() {
        return "No resources are linked to this service\n".to_string();
    }
    let mut output = Vec::new();
    output.push(get_secrets_table_beta(
        &resources
            .iter()
            .filter(|r| matches!(r.r#type, ResourceTypeBeta::Secrets))
            .map(Clone::clone)
            .collect::<Vec<_>>(),
        service_name,
        raw,
    ));
    output.push(get_databases_table_beta(
        &resources
            .iter()
            .filter(|r| {
                matches!(
                    r.r#type,
                    ResourceTypeBeta::DatabaseSharedPostgres
                        | ResourceTypeBeta::DatabaseAwsRdsMariaDB
                        | ResourceTypeBeta::DatabaseAwsRdsMySql
                        | ResourceTypeBeta::DatabaseAwsRdsPostgres
                )
            })
            .map(Clone::clone)
            .collect::<Vec<_>>(),
        service_name,
        raw,
        show_secrets,
    ));
    output.join("\n")
}

fn get_databases_table_beta(
    databases: &[ResourceResponseBeta],
    service_name: &str,
    raw: bool,
    show_secrets: bool,
) -> String {
    if databases.is_empty() {
        return String::new();
    }

    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Type", "Connection string"]);

    for database in databases {
        let connection_string = serde_json::from_value::<DatabaseInfoBeta>(database.output.clone())
            .expect("resource data to be a valid database")
            .connection_string(show_secrets);

        table.add_row(vec![database.r#type.to_string(), connection_string]);
    }

    let show_secret_hint = if databases.is_empty() || show_secrets {
        ""
    } else {
        "Hint: you can show the secrets of these resources using `shuttle resource list --show-secrets`\n"
    };

    format!("These databases are linked to {service_name}\n{table}\n{show_secret_hint}")
}

fn get_secrets_table_beta(
    secrets: &[ResourceResponseBeta],
    service_name: &str,
    raw: bool,
) -> String {
    let Some(secrets) = secrets.first() else {
        return String::new();
    };
    let secrets = serde_json::from_value::<SecretStore>(secrets.output.clone()).unwrap();
    if secrets.secrets.is_empty() {
        return String::new();
    }

    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Key"]);

    for key in secrets.secrets.keys() {
        table.add_row(vec![key]);
    }

    format!("These secrets can be accessed by {service_name}\n{table}")
}

#[cfg(test)]
mod test {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn to_string_and_back() {
        let inputs = [
            ResourceTypeBeta::DatabaseSharedPostgres,
            ResourceTypeBeta::Secrets,
            ResourceTypeBeta::Container,
        ];

        for input in inputs {
            let actual = ResourceTypeBeta::from_str(&input.to_string()).unwrap();
            assert_eq!(input, actual, ":{} should map back to itself", input);
        }
    }
}
