use std::str::FromStr;

use chrono::{DateTime, Local, SecondsFormat};
use comfy_table::{
    presets::{NOTHING, UTF8_BORDERS_ONLY},
    Attribute, Cell, Color, ContentArrangement, Table,
};

use crate::{
    models::{
        certificate::CertificateResponse,
        deployment::DeploymentResponseBeta,
        project::ProjectResponseBeta,
        resource::{ResourceResponseBeta, ResourceTypeBeta},
    },
    secrets::SecretStore,
    DatabaseInfoBeta,
};

pub fn get_certificates_table_beta(certs: &[CertificateResponse], raw: bool) -> String {
    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Certificate ID", "Subject", "Expires"]);

    for cert in certs {
        table.add_row(vec![
            Cell::new(&cert.id).add_attribute(Attribute::Bold),
            Cell::new(&cert.subject),
            Cell::new(&cert.not_after),
        ]);
    }

    table.to_string()
}

pub fn deployments_table_beta(deployments: &[DeploymentResponseBeta], raw: bool) -> String {
    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Deployment ID", "Status", "Date", "Git revision"]);

    for deploy in deployments.iter() {
        let datetime: DateTime<Local> = DateTime::from(deploy.created_at);
        table.add_row(vec![
            Cell::new(&deploy.id).add_attribute(Attribute::Bold),
            Cell::new(&deploy.state)
                // Unwrap is safe because Color::from_str returns the color white if str is not a Color.
                .fg(Color::from_str(deploy.state.get_color()).unwrap()),
            Cell::new(datetime.to_rfc3339_opts(SecondsFormat::Secs, false)),
            Cell::new(
                deploy
                    .build_meta
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ),
        ]);
    }

    table.to_string()
}

pub fn get_projects_table_beta(projects: &[ProjectResponseBeta], raw: bool) -> String {
    let mut table = Table::new();
    table
        .load_preset(if raw { NOTHING } else { UTF8_BORDERS_ONLY })
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_header(vec!["Project ID", "Project Name", "Deployment Status"]);

    for project in projects {
        let state = project
            .deployment_state
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_default();
        let color = project
            .deployment_state
            .as_ref()
            .map(|s| s.get_color())
            .unwrap_or_default();
        table.add_row(vec![
            Cell::new(&project.id).add_attribute(Attribute::Bold),
            Cell::new(&project.name),
            Cell::new(state)
                // Unwrap is safe because Color::from_str returns the color white if the argument is not a Color.
                .fg(Color::from_str(color).unwrap()),
        ]);
    }

    table.to_string()
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
