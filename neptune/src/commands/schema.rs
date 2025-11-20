use anyhow::Result;
use cargo_shuttle::args::OutputMode;

use crate::{ui::AiUi, Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn project_schema(&self) -> Result<NeptuneCommandOutput> {
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);

        let schema = self
            .client
            .api_client
            .get_json::<serde_json::Value>("/schemas/project")
            .await?
            .into_inner();
        let pretty_schema = serde_json::to_string_pretty(&schema)?;

        if self.global_args.output_mode != OutputMode::Json {
            ui.header("Neptune.json schema");
            eprintln!();
        }

        println!("{pretty_schema}");

        Ok(NeptuneCommandOutput::None)
    }
}
