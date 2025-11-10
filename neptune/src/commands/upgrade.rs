#[cfg(target_family = "unix")]
use anyhow::Context;
use anyhow::Result;

use crate::{Neptune, NeptuneCommandOutput};

impl Neptune {
    pub async fn check_upgrade(&self) -> Result<()> {
        todo!()
    }

    pub async fn self_upgrade(&self, _preview: bool) -> Result<NeptuneCommandOutput> {
        // Taken from cargo_shuttle::util::update_cargo_shuttle
        // TODO: preview arg

        #[cfg(target_family = "unix")]
        let _ = tokio::process::Command::new("bash")
            .args(["-c", "curl -sSfL https://www.neptune.dev/install | bash"])
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn bash update process")?
            .wait()
            .await
            .context("Failed to wait on bash update process")?;

        #[cfg(target_family = "windows")]
        let _ = tokio::process::Command::new("powershell")
            .args(["-Command", "iwr https://www.neptune.dev/install-win | iex"])
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn powershell update process")?
            .wait()
            .await
            .context("Failed to wait on powershell update process")?;

        Ok(NeptuneCommandOutput::None)
    }
}
