use std::path::Path;
use std::process::Command;

use semver::Version;

use anyhow::{anyhow, Context, Result};

lazy_static::lazy_static! {
    pub static ref SEMVER_REGEX: regex::Regex = regex::Regex::new(r"shuttle-service v(([0-9]+)\.([0-9]+)\.([0-9]+)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+[0-9A-Za-z-]+)?)").unwrap();
}

pub fn get_shuttle_service_from_user_crate<P: AsRef<Path>>(manifest_path: P) -> Result<Version> {
    let output = Command::new("cargo")
        .args(["tree", "--manifest-path"])
        .arg(manifest_path.as_ref())
        .args([
            "--package",
            "shuttle-service",
            "--depth",
            "0",
            "--edges",
            "normal",
            "--format",
            "{p}",
        ])
        .output()
        .unwrap();

    if !output.status.success() {
        return Err(anyhow!("{}", String::from_utf8_lossy(&output.stderr)))
            .context("`cargo tree --package shuttle-service` failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    let version = SEMVER_REGEX
        .captures(stdout.as_ref())
        .ok_or_else(|| anyhow!("could not figure out the shuttle-service version for deployment"))?
        .get(1)
        .unwrap()
        .as_str();

    Version::parse(version).with_context(|| anyhow!("could not parse {version} as a semver"))
}
