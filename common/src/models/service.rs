use crate::{
    models::{deployment, secret},
    resource::{self, Type},
    DatabaseReadyInfo, SecretStore,
};

use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement,
    Table,
};
use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, path::PathBuf};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub id: Uuid,
    pub name: String,
}

#[derive(Deserialize, Serialize)]
pub struct Detailed {
    pub name: String,
    pub deployments: Vec<deployment::Response>,
    pub resources: Vec<resource::Response>,
    pub secrets: Vec<secret::Response>,
}

#[derive(Deserialize, Serialize)]
pub struct Summary {
    pub name: String,
    pub deployment: Option<deployment::Response>,
    pub resources: Vec<resource::Response>,
    pub uri: String,
}

impl Display for Detailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let deploys = get_deployments_table(&self.deployments, &self.name);
        let resources = get_resources_table(&self.resources);
        let secrets = secret::get_table(&self.secrets);

        write!(f, "{deploys}{resources}{secrets}")
    }
}

impl Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let deployment = if let Some(ref deployment) = self.deployment {
            format!(
                r#"
Service Name:  {}
Deployment ID: {}
Status:        {}
Last Updated:  {}
URI:           {}

"#,
                self.name,
                deployment.id,
                deployment
                    .state
                    .to_string()
                    .with(deployment.state.get_color()),
                deployment.last_update.format("%Y-%m-%dT%H:%M:%SZ"),
                self.uri,
            )
        } else {
            format!(
                "{}\n\n",
                "No deployment is currently running for this service"
                    .yellow()
                    .bold()
            )
        };

        let resources = get_resources_table(&self.resources);

        write!(f, "{deployment}{resources}")
    }
}

fn get_deployments_table(deployments: &Vec<deployment::Response>, service_name: &str) -> String {
    if deployments.is_empty() {
        format!(
            "{}\n",
            "No deployments are linked to this service".yellow().bold()
        )
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("ID").set_alignment(CellAlignment::Center),
                Cell::new("Status").set_alignment(CellAlignment::Center),
                Cell::new("Last updated").set_alignment(CellAlignment::Center),
            ]);

        for deploy in deployments.iter() {
            table.add_row(vec![
                Cell::new(deploy.id),
                Cell::new(&deploy.state)
                    .fg(deploy.state.get_color())
                    .set_alignment(CellAlignment::Center),
                Cell::new(deploy.last_update.format("%Y-%m-%dT%H:%M:%SZ"))
                    .set_alignment(CellAlignment::Center),
            ]);
        }

        format!(
            r#"
Most recent deploys for {}
{}

"#,
            service_name.bold(),
            table,
        )
    }
}

pub fn get_resources_table(resources: &Vec<resource::Response>) -> String {
    if resources.is_empty() {
        format!("{}\n", "No resources are linked to this service".bold())
    } else {
        let resource_groups = resources.iter().fold(HashMap::new(), |mut acc, x| {
            let title = match x.r#type {
                Type::Database(_) => "Databases",
                Type::Secrets => "Secrets",
                Type::StaticFolder => "Static Folder",
                Type::Persist => "Persist",
            };

            let elements = acc.entry(title).or_insert(Vec::new());
            elements.push(x);

            acc
        });

        let mut output = Vec::new();

        if let Some(databases) = resource_groups.get("Databases") {
            output.push(get_databases_table(databases));
        };

        if let Some(secrets) = resource_groups.get("Secrets") {
            output.push(get_secrets_table(secrets));
        };

        if let Some(static_folders) = resource_groups.get("Static Folder") {
            output.push(get_static_folder_table(static_folders));
        };

        if let Some(persist) = resource_groups.get("Persist") {
            output.push(get_persist_table(persist));
        };

        output.join("\n")
    }
}

fn get_databases_table(databases: &Vec<&resource::Response>) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_header(vec![
            Cell::new("Type").set_alignment(CellAlignment::Center),
            Cell::new("Connection string").set_alignment(CellAlignment::Center),
        ]);

    for database in databases {
        let info = serde_json::from_value::<DatabaseReadyInfo>(database.data.clone()).unwrap();

        table.add_row(vec![
            database.r#type.to_string(),
            info.connection_string_public(),
        ]);
    }

    format!(
        r#"These databases are linked to this service
{table}
"#,
    )
}

fn get_secrets_table(secrets: &[&resource::Response]) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![Cell::new("Key").set_alignment(CellAlignment::Center)]);

    let secrets = serde_json::from_value::<SecretStore>(secrets[0].data.clone()).unwrap();

    for key in secrets.secrets.keys() {
        table.add_row(vec![key]);
    }

    format!(
        r#"These secrets can be accessed by the service
{table}
"#,
    )
}

fn get_static_folder_table(static_folders: &[&resource::Response]) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_header(vec![
            Cell::new("Static Folders").set_alignment(CellAlignment::Center)
        ]);

    for folder in static_folders {
        let path = serde_json::from_value::<PathBuf>(folder.data.clone())
            .unwrap()
            .display()
            .to_string();

        table.add_row(vec![path]);
    }

    format!(
        r#"These static folders can be accessed by the service
{table}
"#,
    )
}

fn get_persist_table(persist_instances: &[&resource::Response]) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_header(vec![
            Cell::new("Persist Instances").set_alignment(CellAlignment::Center)
        ]);

    for _ in persist_instances {
        table.add_row(vec!["Instance"]);
    }

    format!(
        r#"These instances are linked to this service
{table}
"#,
    )
}
