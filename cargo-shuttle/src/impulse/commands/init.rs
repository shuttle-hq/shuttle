use anyhow::Result;

use crate::impulse::{args::InitArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn init(&self, _init_args: InitArgs) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        // TODO: offer to log in if not done yet?
        // TODO: offer to generate agents.md
        // TODO: copy init login from cargo-shuttle
        Ok(ImpulseCommandOutput::None)
    }
}
