use std::{
    process::{Child, Command},
    time::Duration,
};

use tempdir::TempDir;

pub struct Api {
    process: Child,
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

        Self { process }
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        self.process.kill().unwrap();
    }
}
