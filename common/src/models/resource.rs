use std::collections::HashMap;

use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS,
    presets::{NOTHING, UTF8_BORDERS_ONLY, UTF8_FULL},
    Attribute, Cell, CellAlignment, ContentArrangement, Table,
};
use crossterm::style::Stylize;

use crate::{
    certificate::CertificateResponse,
    resource::{Response, Type},
    secrets::SecretStore,
    DatabaseInfoBeta, DatabaseResource,
};

pub fn get_resource_tables(
    resources: &[Response],
    service_name: &str,
    raw: bool,
    show_secrets: bool,
    beta: bool,
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
                Type::Persist => "Persist",
                // ignore variants that don't represent Shuttle-hosted resources
                Type::Container => return acc,
            };

            let elements: &mut Vec<_> = acc.entry(title).or_default();
            elements.push(x);

            acc
        });

        let mut output = Vec::new();

        if let Some(databases) = resource_groups.get("Databases") {
            if beta {
                output.push(get_databases_table_beta(
                    databases,
                    service_name,
                    raw,
                    show_secrets,
                ));
            } else {
                output.push(get_databases_table(
                    databases,
                    service_name,
                    raw,
                    show_secrets,
                ));
            }
        };

        if let Some(secrets) = resource_groups.get("Secrets") {
            output.push(get_secrets_table(secrets, service_name, raw));
        };

        if resource_groups.contains_key("Persist") {
            output.push(format!("This persist instance is linked to {service_name}\nShuttle Persist: {service_name}\n"));
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
        let info = serde_json::from_value::<DatabaseResource>(database.data.clone())
            .expect("resource data to be a valid database");
        let conn_string = match info {
            DatabaseResource::ConnectionString(url) => {
                if let Ok(mut url) = url.parse::<url::Url>() {
                    // if the local_uri can correctly be parsed as a url,
                    // hide the password before producing table,
                    // since it contains an interpolated secret or a hardcoded password
                    if url.password().is_some() {
                        // ignore edge cases (if any)
                        let _ = url.set_password(Some("********"));
                    }
                    url.to_string()
                } else {
                    url
                }
            }
            DatabaseResource::Info(info) => {
                if info.hostname_shuttle == "localhost" && info.hostname_public == "localhost" {
                    // If both hostnames are localhost, this must be a local container
                    // from the local provisioner with a default password.
                    // (DatabaseResource::Info is always from a provisioner server)
                    // It is revealed since this is the only place to see local db url.
                    info.connection_string_public(true)
                } else {
                    info.connection_string_public(show_secrets)
                }
            }
        };
        table.add_row(vec![database.r#type.to_string(), conn_string]);
    }

    let show_secret_hint = if databases.is_empty() || show_secrets {
        ""
    } else {
        "Hint: you can show the secrets of these resources using `cargo shuttle resource list --show-secrets`\n"
    };

    format!("These databases are linked to {service_name}\n{table}\n{show_secret_hint}")
}

fn get_databases_table_beta(
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
            .load_preset(UTF8_BORDERS_ONLY)
            .set_content_arrangement(ContentArrangement::Disabled)
            .set_header(vec![Cell::new("Type"), Cell::new("Connection string")]);
    }

    for database in databases {
        let connection_string = serde_json::from_value::<DatabaseInfoBeta>(database.data.clone())
            .expect("resource data to be a valid database")
            .connection_string(show_secrets);

        table.add_row(vec![database.r#type.to_string(), connection_string]);
    }

    let show_secret_hint = if databases.is_empty() || show_secrets {
        ""
    } else {
        "Hint: you can show the secrets of these resources using `cargo shuttle resource list --show-secrets`\n"
    };

    format!("These databases are linked to {service_name}\n{table}\n{show_secret_hint}")
}

pub fn get_certificates_table_beta(certs: &[CertificateResponse], raw: bool) -> String {
    let mut table = Table::new();

    if raw {
        table
            .load_preset(NOTHING)
            .set_content_arrangement(ContentArrangement::Disabled)
            .set_header(vec![
                Cell::new("ID").set_alignment(CellAlignment::Left),
                Cell::new("Subject").set_alignment(CellAlignment::Left),
                Cell::new("Expires").set_alignment(CellAlignment::Left),
            ]);
    } else {
        table
            .load_preset(UTF8_BORDERS_ONLY)
            .set_content_arrangement(ContentArrangement::Disabled)
            .set_header(vec![
                Cell::new("ID"),
                Cell::new("Subject"),
                Cell::new("Expires"),
            ]);
    }

    for cert in certs {
        table.add_row(vec![
            cert.id.clone(),
            cert.subject.clone(),
            cert.not_after.clone(),
        ]);
    }

    table.to_string()
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
