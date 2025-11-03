use anyhow::Result;

use crate::impulse::{Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn status(&self) -> Result<ImpulseCommandOutput> {
        let spec = match self.fetch_local_state().await {
            Ok(project) => project,
            Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!("'shuttle.json' not found: the project manifest was not generated");
                return Ok(ImpulseCommandOutput::None);
            }
            _ => return Ok(ImpulseCommandOutput::None),
        };

        let projects = self.client.get_impulse_projects().await?.into_inner();

        if let Some(ref status) = projects
            .into_iter()
            .find(|x| x.name == spec.name())
            .and_then(|p| p.status)
        {
            Ok(ImpulseCommandOutput::ProjectStatus(Box::new(
                status.clone(),
            )))
        } else {
            tracing::warn!("Remote project not found: the project was not deployed");
            Ok(ImpulseCommandOutput::None)
        }
    }
}
