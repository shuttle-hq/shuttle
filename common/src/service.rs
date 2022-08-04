use std::fmt::Display;

use colored::Colorize;
use comfy_table::{
    modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, CellAlignment, ContentArrangement,
    Table,
};
use serde::{Deserialize, Serialize};

use crate::{
    deployment,
    resource::{self, ResourceInfo},
};

#[derive(Deserialize, Serialize)]
pub struct Response {
    pub name: String,
    pub deployments: Vec<deployment::Response>,
    pub resources: Vec<resource::Response>,
    pub uri: String,
}

#[derive(Deserialize, Serialize)]
pub struct Summary {
    pub name: String,
    pub deployment: Option<deployment::Response>,
    pub resources: Vec<resource::Response>,
    pub uri: String,
}

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let deploys = get_deployments_table(&self.deployments, &self.name);
        let resources = get_resources_table(&self.resources);

        write!(f, "{}{}", deploys, resources)
    }
}

impl Display for Summary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let deployment = if let Some(ref deployment) = self.deployment {
            format!(
                "Deployment '{}' has been {} since {}\n\n",
                deployment.id,
                deployment.state.to_string().cyan(),
                deployment
                    .last_update
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string()
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

        write!(f, "{}{}", deployment, resources)
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

fn get_resources_table(resources: &Vec<resource::Response>) -> String {
    if resources.is_empty() {
        format!("{}\n", "No resources are linked to this service".bold())
    } else {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("Type").set_alignment(CellAlignment::Center),
                Cell::new("Connection string").set_alignment(CellAlignment::Center),
            ]);

        for resource in resources.iter() {
            table.add_row(vec![
                resource.r#type.to_string(),
                resource.get_resource_info().connection_string_public(),
            ]);
        }

        format!(
            r#"These resources are linked to this service
{}
"#,
            table
        )
    }
}
