use anyhow::Result;

use crate::impulse::{args::DeployArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn deploy(&self, _deploy_args: DeployArgs) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }
}
