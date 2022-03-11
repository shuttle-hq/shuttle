use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::process::{ExitStatus, Stdio};
use std::thread::sleep;
use std::{
    io::{self, BufRead},
    process::{Child, Command},
    str,
    time::Duration,
    time::SystemTime,
};

use colored::*;
use portpicker::pick_unused_port;
use reqwest::blocking::RequestBuilder;

trait EnsureSuccess {
    fn ensure_success<S: AsRef<str>>(self, s: S);
}

impl EnsureSuccess for io::Result<ExitStatus> {
    fn ensure_success<S: AsRef<str>>(self, s: S) {
        let exit_status = self.unwrap();
        if !exit_status.success() {
            panic!("{}: exit code {}", s.as_ref(), exit_status)
        }
    }
}

pub struct Server {
    process: Child,
}

pub struct Api {
    server: Option<Server>,
    api_addr: SocketAddr,
    proxy_addr: SocketAddr,
    target: String,
    color: Color,
}

pub fn log_lines<R: io::Read, D: std::fmt::Display>(mut reader: R, target: D) {
    let mut buf = [0; 2 << 17]; // 128kb
    let mut current_pos = 0;
    loop {
        let n = reader.read(&mut buf[current_pos..]).unwrap();
        if n == 0 {
            break;
        }
        current_pos += n;

        if buf[current_pos - 1] != b'\n' {
            continue;
        }

        for line in io::BufReader::new(&buf[..current_pos]).lines() {
            eprintln!("{} {}", target, line.unwrap());
        }

        current_pos = 0;
    }

    // Log last
    if current_pos != 0 {
        for line in io::BufReader::new(&buf[..current_pos]).lines() {
            eprintln!("{} {}", target, line.unwrap());
        }
    }
}

pub fn spawn_and_log<D: std::fmt::Display, C: Into<Color>>(
    cmd: &mut Command,
    target: D,
    color: C,
) -> Child {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let color = color.into();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let stdout_target = format!("{} >>>", target).color(color.clone());
    let stderr_target = format!("{} >>>", target).bold().color(color);
    std::thread::spawn(move || log_lines(&mut stdout, stdout_target));
    std::thread::spawn(move || log_lines(&mut stderr, stderr_target));
    child
}

impl Api {
    fn new_free<D, C>(target: D, color: C) -> Self
    where
        D: std::fmt::Display,
        C: Into<Color>,
    {
        let api_port = pick_unused_port().expect("could not find a free port for API");

        let api_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, api_port).into();

        let proxy_port = pick_unused_port().expect("could not find a free port for proxy");

        let proxy_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, proxy_port).into();

        Self {
            server: None,
            api_addr,
            proxy_addr,
            target: target.to_string(),
            color: color.into(),
        }
    }

    pub fn new_docker<D, C>(target: D, color: C) -> Self
    where
        D: std::fmt::Display,
        C: Into<Color>,
    {
        let mut api = Self::new_free(target, color);

        let api_target = format!("   {} api", api.target);

        let mut build = Command::new("docker");

        build
            .args(["build", "-f", "./Dockerfile", "."])
            .current_dir("../");

        spawn_and_log(&mut build, api_target.as_str(), Color::White)
            .wait()
            .ensure_success("failed to build `api` image");

        let output = Command::new("docker")
            .args(["build", "-q", "-f", "./Dockerfile", "."])
            .current_dir("../")
            .output()
            .unwrap();

        let image = String::from_utf8(output.stdout).unwrap().trim().to_owned();

        let mut run = Command::new("docker");
        run.args([
            "run",
            "-i",
            "--rm",
            "-p",
            format!("{}:{}", api.proxy_addr.port(), 8000).as_str(),
            "-p",
            format!("{}:{}", api.api_addr.port(), 8001).as_str(),
            "-e",
            "PROXY_PORT=8000",
            "-e",
            "API_PORT=8001",
            "-e",
            "UNVEIL_USERS_TOML=/config/users.toml",
            "-v",
            &format!(
                "{}/users.toml:/config/users.toml",
                env!("CARGO_MANIFEST_DIR")
            ),
            image.as_str(),
        ]);

        let child = spawn_and_log(&mut run, api_target, api.color);

        api.server = Some(Server { process: child });

        api.wait_ready(Duration::from_secs(120));

        api
    }

    pub fn wait_ready(&self, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            match reqwest::blocking::get(format!("http://{}/status", self.api_addr)) {
                Ok(resp) if resp.status().is_success() => return,
                _ => sleep(Duration::from_secs(1)),
            }
            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for api to /status OK");
    }

    pub fn run_client<'s, I>(&self, args: I, path: &str) -> Child
    where
        I: IntoIterator<Item = &'s str>,
    {
        let client_target = format!("{} client", self.target);

        let mut build = Command::new("cargo");
        build
            .args(["build", "--bin", "cargo-unveil"])
            .current_dir("../");
        spawn_and_log(&mut build, client_target.as_str(), Color::White)
            .wait()
            .ensure_success("failed to build `cargo-unveil`");

        let mut run = Command::new("../../../target/debug/cargo-unveil");
        run.args(args)
            .current_dir(path)
            .env("UNVEIL_API", format!("http://{}", self.api_addr));
        spawn_and_log(&mut run, client_target, self.color)
    }

    pub fn deploy(&self, project_path: &str) {
        self.run_client(["deploy", "--allow-dirty"], project_path)
            .wait()
            .ensure_success("failed to run deploy");
    }

    pub fn get(&self, sub_path: &str) -> RequestBuilder {
        reqwest::blocking::Client::new().get(format!("http://{}/{}", self.proxy_addr, sub_path))
    }

    pub fn post(&self, sub_path: &str) -> RequestBuilder {
        reqwest::blocking::Client::new().post(format!("http://{}/{}", self.proxy_addr, sub_path))
    }
}

impl Drop for Api {
    fn drop(&mut self) {
        if let Some(server) = self.server.as_mut() {
            server.process.kill().unwrap();
        }
    }
}
