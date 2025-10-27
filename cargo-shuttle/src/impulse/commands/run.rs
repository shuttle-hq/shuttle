use anyhow::Result;

use crate::impulse::{args::RunArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn local_run(&self, _run_args: RunArgs) -> Result<ImpulseCommandOutput> {
        unimplemented!();
        Ok(ImpulseCommandOutput::None)
    }
}
