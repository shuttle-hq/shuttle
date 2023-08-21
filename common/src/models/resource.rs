use std::collections::HashMap;

use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Attribute, Cell, CellAlignment,
    ContentArrangement, Table,
};
use crossterm::style::Stylize;

use crate::{
    resource::{Response, Type},
    DbOutput, SecretStore,
};

pub fn get_resources_table(resources: &Vec<Response>, service_name: &str) -> String {
    if resources.is_empty() {
        format!("{}\n", "No resources are linked to this service".bold())
    } else {
        let resource_groups = resources.iter().fold(HashMap::new(), |mut acc, x| {
            let title = match x.r#type {
                Type::Database(_) => "Databases",
                Type::Secrets => "Secrets",
                Type::StaticFolder => "Static Folder",
                Type::Persist => "Persist",
                Type::Turso => "Turso",
                Type::ServiceInfo => "Service Info",
                Type::Custom => "Custom",
            };

            let elements = acc.entry(title).or_insert(Vec::new());
            elements.push(x);

            acc
        });

        let mut output = Vec::new();

        if let Some(databases) = resource_groups.get("Databases") {
            output.push(get_databases_table(databases, service_name));
        };

        if let Some(secrets) = resource_groups.get("Secrets") {
            output.push(get_secrets_table(secrets, service_name));
        };

        if let Some(static_folders) = resource_groups.get("Static Folder") {
            output.push(get_static_folder_table(static_folders, service_name));
        };

        if let Some(persist) = resource_groups.get("Persist") {
            output.push(get_persist_table(persist, service_name));
        };

        if let Some(custom) = resource_groups.get("Custom") {
            output.push(get_custom_resources_table(custom, service_name));
        };

        output.join("\n")
    }
}

fn get_databases_table(databases: &Vec<&Response>, service_name: &str) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth)
        .set_header(vec![
            Cell::new("Type")
                .add_attribute(Attribute::Bold)
                .set_alignment(CellAlignment::Center),
            Cell::new("Connection string")
                .add_attribute(Attribute::Bold)
                .set_alignment(CellAlignment::Center),
        ]);

    for database in databases {
        let info = serde_json::from_value::<DbOutput>(database.data.clone()).unwrap();
        let connection_string = match info {
            DbOutput::Local(local_uri) => local_uri.clone(),
            DbOutput::Info(info) => info.connection_string_public(),
        };

        table.add_row(vec![database.r#type.to_string(), connection_string]);
    }

    format!(
        r#"These {} are linked to {}
{table}
"#,
        "databases".bold(),
        service_name
    )
}

fn get_secrets_table(secrets: &[&Response], service_name: &str) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![Cell::new("Keys")
            .add_attribute(Attribute::Bold)
            .set_alignment(CellAlignment::Center)]);

    let secrets = serde_json::from_value::<SecretStore>(secrets[0].data.clone()).unwrap();

    for key in secrets.secrets.keys() {
        table.add_row(vec![key]);
    }

    format!(
        r#"These {} can be accessed by {}
{table}
"#,
        "secrets".bold(),
        service_name
    )
}

fn get_static_folder_table(static_folders: &[&Response], service_name: &str) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![Cell::new("Folders")
            .set_alignment(CellAlignment::Center)
            .add_attribute(Attribute::Bold)]);

    for folder in static_folders {
        let path = serde_json::from_value::<String>(folder.config.clone()).unwrap();

        table.add_row(vec![path]);
    }

    format!(
        r#"These {} can be accessed by {}
{table}
"#,
        "static folders".bold(),
        service_name
    )
}

fn get_persist_table(persist_instances: &[&Response], service_name: &str) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![Cell::new("Instances")
            .set_alignment(CellAlignment::Center)
            .add_attribute(Attribute::Bold)]);

    for _ in persist_instances {
        table.add_row(vec!["Instance"]);
    }

    format!(
        r#"These {} are linked to {}
{table}
"#,
        "persist instances".bold(),
        service_name
    )
}

fn get_custom_resources_table(
    custom_resource_instances: &[&Response],
    service_name: &str,
) -> String {
    let mut table = Table::new();

    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec![Cell::new("Instances")
            .set_alignment(CellAlignment::Center)
            .add_attribute(Attribute::Bold)]);

    for (idx, _) in custom_resource_instances.iter().enumerate() {
        // TODO: add some information that would make the custom resources identifiable.
        // This requires changing the backend resource list response to include a resource identifier
        // that can be used to query for more info related to a resource.
        table.add_row(vec![format!("custom-resource-{}", idx.to_string())]);
    }

    format!(
        r#"These {} are linked to {}
{table}
"#,
        "custom resource instances".bold(),
        service_name
    )
}
