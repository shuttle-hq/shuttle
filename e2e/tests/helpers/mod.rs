use std::{
    process::{Child, Command},
    str,
    time::Duration,
};

use tempdir::TempDir;

pub struct Api {
    process: Child,
    tmp_dir: TempDir,
}

impl Api {
    pub fn new() -> Self {
        let tmp_dir = TempDir::new("e2e").unwrap();

        // Spawn into background
        let process = Command::new("cargo")
            .args([
                "run",
                "--bin",
                "api",
                "--",
                "--path",
                tmp_dir.path().to_str().unwrap(),
            ])
            .current_dir("../")
            .spawn()
            .unwrap();

        std::thread::sleep(Duration::from_secs(1));

        Self { process, tmp_dir }
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        self.process.kill().unwrap();
        std::fs::remove_dir_all(self.tmp_dir.path()).unwrap();
    }
}

pub fn deploy(project_path: &str) {
    let unveil_output = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "cargo-unveil",
            "--manifest-path",
            "../../../Cargo.toml",
            "--",
            "deploy",
        ])
        .current_dir(project_path)
        .output()
        .unwrap();

    let stdout = str::from_utf8(&unveil_output.stdout).unwrap();
    assert!(
        stdout.contains("Finished dev"),
        "output does not contain 'Finished dev':\nstdout = {}\nstderr = {}",
        stdout,
        str::from_utf8(&unveil_output.stderr).unwrap()
    );
    assert!(stdout.contains("Deployment Status:  DEPLOYED"));
}
