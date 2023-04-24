use crossterm::style::Stylize;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::deployment;

#[derive(Deserialize, Serialize, ToSchema)]
#[schema(as = shuttle_common::models::service::Response)]
pub struct Response {
    #[schema(value_type = KnownFormat::Uuid)]
    pub id: Uuid,
    pub name: String,
}

#[derive(Deserialize, Serialize, ToSchema)]
#[schema(as = shuttle_common::models::service::Summary)]
pub struct Summary {
    pub name: String,
    #[schema(value_type = shuttle_common::models::deployment::Response)]
    pub deployment: Option<deployment::Response>,
    pub uri: String,
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
                self.name.clone().bold(),
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

        write!(f, "{deployment}")
    }
}
