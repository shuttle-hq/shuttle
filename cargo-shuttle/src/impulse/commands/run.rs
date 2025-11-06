use anyhow::Result;

use crate::impulse::{args::RunArgs, Impulse, ImpulseCommandOutput};

impl Impulse {
    pub async fn local_run(&self, _run_args: RunArgs) -> Result<ImpulseCommandOutput> {
        // let image_name = self.build(_run_args.build_args).await?;

        // TODO: local run with docker
        Ok(ImpulseCommandOutput::None)
    }
}
