use std::{
    process::{Child, Command},
    str,
    io,
    time::Duration,
};
use std::process::Output;
use std::ffi::OsStr;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use tempdir::TempDir;
use portpicker::pick_unused_port;

pub struct Api {
    process: Child,
    tmp_dir: TempDir,
    api_addr: SocketAddr,
    proxy_addr: SocketAddr,
}

impl Api {
    pub fn new() -> Self {
        let tmp_dir = TempDir::new("e2e").unwrap();

        let api_port = pick_unused_port()
            .expect("could not find a free port for API");

        let api_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, api_port)
            .into();

        let proxy_port = pick_unused_port()
            .expect("could not find a free port for proxy");

        let proxy_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, proxy_port)
            .into();

        // Spawn into background
        let process = Command::new("cargo")
            .args([
                "run",
                "--bin",
                "api",
                "--",
                "--path",
                tmp_dir.path().to_str().unwrap(),
                "--api-port",
                api_port.to_string().as_str(),
                "--proxy-port",
                proxy_port.to_string().as_str()
            ])
            .current_dir("../")
            .spawn()
            .unwrap();

        std::thread::sleep(Duration::from_secs(1));

        Self { process, tmp_dir, api_addr, proxy_addr }
    }

    pub fn run_client<'s, I>(&self, args: I, project_path: &str) -> io::Result<Output>
        where
            I: IntoIterator<Item = &'s str>,
    {
        Command::new("cargo")
            .args([
                "run",
                "--bin",
                "cargo-unveil",
                "--manifest-path",
                "../../../Cargo.toml",
                "--"
            ].into_iter().chain(args))
            .current_dir(project_path)
            .env("UNVEIL_API", format!("http://{}", self.api_addr))
            .output()
    }

    pub fn deploy(&self, project_path: &str) {
        let unveil_output = self.run_client(["deploy"], project_path).unwrap();

        let stdout = str::from_utf8(&unveil_output.stdout).unwrap();
        assert!(
            stdout.contains("Finished dev"),
            "output does not contain 'Finished dev':\nstdout = {}\nstderr = {}",
            stdout,
            str::from_utf8(&unveil_output.stderr).unwrap()
        );
        assert!(stdout.contains("Deployment Status:  DEPLOYED"));
    }

}

impl Drop for Api {
    fn drop(&mut self) {
        self.process.kill().unwrap();
        std::fs::remove_dir_all(self.tmp_dir.path()).unwrap();
    }
}