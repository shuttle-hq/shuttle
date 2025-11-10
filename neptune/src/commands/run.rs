use anyhow::Result;

use crate::{args::RunArgs, Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn local_run(&self, _run_args: RunArgs) -> Result<NeptuneCommandOutput> {
        // let image_name = self.build(_run_args.build_args).await?;

        // TODO: local run with docker
        Ok(NeptuneCommandOutput::None)
    }
}
