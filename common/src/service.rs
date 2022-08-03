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

impl Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut deploys = Table::new();
        deploys
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("ID").set_alignment(CellAlignment::Center),
                Cell::new("Status").set_alignment(CellAlignment::Center),
                Cell::new("Last updated").set_alignment(CellAlignment::Center),
            ]);

        for deploy in self.deployments.iter() {
            deploys.add_row(vec![
                Cell::new(deploy.id),
                Cell::new(&deploy.state)
                    .fg(deploy.state.get_color())
                    .set_alignment(CellAlignment::Center),
                Cell::new(deploy.last_update.format("%Y-%m-%dT%H:%M:%SZ"))
                    .set_alignment(CellAlignment::Center),
            ]);
        }

        let mut resources = Table::new();
        resources
            .load_preset(UTF8_FULL)
            .apply_modifier(UTF8_ROUND_CORNERS)
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_header(vec![
                Cell::new("Type").set_alignment(CellAlignment::Center),
                Cell::new("Connection string").set_alignment(CellAlignment::Center),
            ]);

        for resource in self.resources.iter() {
            resources.add_row(vec![
                resource.r#type.to_string(),
                resource.get_resource_info().connection_string_public(),
            ]);
        }

        write!(
            f,
            r#"
Most recent deploys for {}
{}

These resources are linked to this service
{}
"#,
            self.name.bold(),
            deploys,
            resources
        )
    }
}
