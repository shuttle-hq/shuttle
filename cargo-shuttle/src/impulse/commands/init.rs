use anyhow::Result;

use crate::impulse::{args::InitArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn init(&self, _init_args: InitArgs) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }
}
