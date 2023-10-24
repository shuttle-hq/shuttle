use std::collections::HashMap;

use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS,
    presets::{NOTHING, UTF8_FULL},
    Attribute, Cell, CellAlignment, ContentArrangement, Table,
};
use crossterm::style::Stylize;

use crate::{
    resource::{Response, Type},
    secrets::SecretStore,
    DbOutput,
};

pub fn get_resources_table(
    resources: &Vec<Response>,
    service_name: &str,
    raw: bool,
    show_secrets: bool,
) -> String {
    if resources.is_empty() {
        if raw {
            "No resources are linked to this service\n".to_string()
        } else {
            format!("{}\n", "No resources are linked to this service".bold())
        }
    } else {
        let resource_groups = resources.iter().fold(HashMap::new(), |mut acc, x| {
            let title = match x.r#type {
                Type::Database(_) => "Databases",
                Type::Secrets => "Secrets",
                Type::StaticFolder => "Static Folder",
                Type::Persist => "Persist",
                Type::Turso => "Turso",
                Type::Metadata => "Metadata",
                Type::Custom => "Custom",
            };

            let elements = acc.entry(title).or_insert(Vec::new());
            elements.push(x);

            acc
        });

        let mut output = Vec::new();

        if let Some(databases) = resource_groups.get("Databases") {
            output.push(get_databases_table(
                databases,
                service_name,
                raw,
                show_secrets,
            ));
        };

        if let Some(secrets) = resource_groups.get("Secrets") {
            output.push(get_secrets_table(secrets, service_name, raw));
        };

        if let Some(static_folders) = resource_groups.get("Static Folder") {
            output.push(get_static_folder_table(static_folders, service_name, raw));
        };

        if let Some(persist) = resource_groups.get("Persist") {
            output.push(get_persist_table(persist, service_name, raw));
        };

        if let Some(custom) = resource_groups.get("Custom") {
            output.push(get_custom_resources_table(custom, service_name, raw));
        };

        output.join("\n")
    }
}

fn get_databases_table(
    databases: &Vec<&Response>,
    service_name: &str,
    raw: bool,
    show_secrets: bool,
) -> String {
    let mut table = Table::new();

    if raw {
        table
            .load_preset(NOTHING)
            .set_content_arrangement(ContentArrangement::Disabled)
            .set_header(vec![
                Cell::new("Type").set_alignment(CellAlignment::Left),
                Cell::new("Connection string").set_alignment(CellAlignment::Left),
            ]);
    } else {
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("Type")
                    .set_alignment(CellAlignment::Center)
                    .add_attribute(Attribute::Bold),
                Cell::new("Connection string")
                    .add_attribute(Attribute::Bold)
                    .set_alignment(CellAlignment::Center),
            ]);
    }

    for database in databases {
        let info = serde_json::from_value::<DbOutput>(database.data.clone()).expect("");
        let conn_string = match info {
            DbOutput::Local(info) => info.clone(),
            DbOutput::Info(local_uri) => {
                if show_secrets {
                    local_uri.connection_string_private()
                } else {
                    local_uri.connection_string_public()
                }
            }
        };
        table.add_row(vec![database.r#type.to_string(), conn_string]);
    }

    let show_secret_hint = if databases.is_empty() || show_secrets {
        ""
    } else {
        "Hint: you can show the secrets of these resources using --show-secrets\n"
    };

    format!(
        "These databases are linked to {service_name}\n{table}\n{}",
        show_secret_hint
    )
}

fn get_secrets_table(secrets: &[&Response], service_name: &str, raw: bool) -> String {
    let mut table = Table::new();

    if raw {
        table
            .load_preset(NOTHING)
            .set_header(vec![Cell::new("Keys").set_alignment(CellAlignment::Left)]);
    } else {
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![Cell::new("Keys")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)]);
    }

    let secrets = serde_json::from_value::<SecretStore>(secrets[0].data.clone()).unwrap();

    for key in secrets.secrets.keys() {
        table.add_row(vec![key]);
    }

    format!("These secrets can be accessed by {service_name}\n{table}\n")
}

fn get_static_folder_table(static_folders: &[&Response], service_name: &str, raw: bool) -> String {
    let mut table = Table::new();

    if raw {
        table
            .load_preset(NOTHING)
            .set_header(vec![Cell::new("Folders").set_alignment(CellAlignment::Left)]);
    } else {
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![Cell::new("Folders")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)]);
    }

    for folder in static_folders {
        let path = serde_json::from_value::<String>(folder.config.clone()).unwrap();

        table.add_row(vec![path]);
    }

    format!("These static folders can be accessed by {service_name}\n{table}\n")
}

fn get_persist_table(persist_instances: &[&Response], service_name: &str, raw: bool) -> String {
    let mut table = Table::new();

    if raw {
        table.load_preset(NOTHING).set_header(vec![
            Cell::new("Instances").set_alignment(CellAlignment::Left)
        ]);
    } else {
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![Cell::new("Instances")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)]);
    }

    for _ in persist_instances {
        table.add_row(vec!["Instance"]);
    }

    format!("These persist instances are linked to {service_name}\n{table}\n")
}

fn get_custom_resources_table(
    custom_resource_instances: &[&Response],
    service_name: &str,
    raw: bool,
) -> String {
    let mut table = Table::new();

    if raw {
        table.load_preset(NOTHING).set_header(vec![
            Cell::new("Instances").set_alignment(CellAlignment::Left)
        ]);
    } else {
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_header(vec![Cell::new("Instances")
                .set_alignment(CellAlignment::Center)
                .add_attribute(Attribute::Bold)]);
    }

    for (idx, _) in custom_resource_instances.iter().enumerate() {
        // TODO: add some information that would make the custom resources identifiable.
        // This requires changing the backend resource list response to include a resource identifier
        // that can be used to query for more info related to a resource.
        table.add_row(vec![format!("custom-resource-{}", idx.to_string())]);
    }

    format!("These custom resource instances are linked to {service_name}\n{table}\n")
}
