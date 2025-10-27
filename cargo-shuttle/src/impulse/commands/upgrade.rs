use anyhow::Result;

use crate::{
    impulse::{Impulse, ImpulseCommandOutput},
    util::update_cargo_shuttle,
};

impl Impulse {
    pub async fn check_upgrade(&self) -> Result<()> {
        todo!()
    }

    pub async fn self_upgrade(&self, preview: bool) -> Result<ImpulseCommandOutput> {
        update_cargo_shuttle(preview).await?;

        Ok(ImpulseCommandOutput::None)
    }
}
