use std::fs::File;
use std::io::{self, BufRead};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::str;
use std::thread::sleep;
use std::time::{Duration, SystemTime};

use colored::*;
use portpicker::pick_unused_port;
use rand::Rng;
use reqwest::blocking::RequestBuilder;

const ID_CHARSET: &[u8] = b"0123456789abcdef";
const ID_LEN: u8 = 8;

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

pub struct Services {
    id: String,
    api_addr: SocketAddr,
    proxy_addr: SocketAddr,
    api_image: Option<String>,
    api_container: Option<String>,
    provisioner_image: Option<String>,
    provisioner_container: Option<String>,
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
    let stdout_target = format!("{} >>>", target).color(color);
    let stderr_target = format!("{} >>>", target).bold().color(color);
    std::thread::spawn(move || log_lines(&mut stdout, stdout_target));
    std::thread::spawn(move || log_lines(&mut stderr, stderr_target));
    child
}

impl Services {
    fn new_free<D, C>(target: D, color: C) -> Self
    where
        D: std::fmt::Display,
        C: Into<Color>,
    {
        let mut rng = rand::thread_rng();
        let id: String = (0..ID_LEN)
            .map(|_| {
                let idx = rng.gen_range(0..ID_CHARSET.len());
                ID_CHARSET[idx] as char
            })
            .collect();

        let api_port = pick_unused_port().expect("could not find a free port for API");

        let api_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, api_port).into();

        let proxy_port = pick_unused_port().expect("could not find a free port for proxy");

        let proxy_addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, proxy_port).into();

        Self {
            id,
            api_addr,
            proxy_addr,
            api_image: None,
            api_container: None,
            provisioner_image: None,
            provisioner_container: None,
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
        let users_toml_file = format!("{}/users.toml", env!("CARGO_MANIFEST_DIR"));

        // Make sure network is up
        Command::new("docker")
            .args(["network", "create", "--driver", "bridge", "shuttle-net"])
            .spawn()
            .unwrap()
            .wait()
            .unwrap();

        let provisioner_image = Self::build_image("provisioner", &api.target, &api.id);
        api.provisioner_image = Some(provisioner_image.clone());

        let provisioner_target = format!("{} provisioner", api.target);
        let provisioner_container = format!("shuttle_provisioner_{}_{}", api.target, api.id);
        let mut run = Command::new("docker");
        run.args([
            "run",
            "--name",
            &provisioner_container,
            "--network",
            "shuttle-net",
            "-e",
            "PORT=5001",
            &provisioner_image,
        ]);
        api.provisioner_container = Some(provisioner_container.clone());

        spawn_and_log(&mut run, provisioner_target, api.color);

        let api_target = format!("        {} api", api.target);
        let api_image = Self::build_image("api", &api.target, &api.id);
        api.api_image = Some(api_image.clone());

        File::create(&users_toml_file).unwrap();

        let api_container = format!("shuttle_api_{}_{}", api.target, api.id);
        let mut run = Command::new("docker");
        run.args([
            "run",
            "--name",
            &api_container,
            "--network",
            "shuttle-net",
            "-p",
            format!("{}:{}", api.proxy_addr.port(), 8000).as_str(),
            "-p",
            format!("{}:{}", api.api_addr.port(), 8001).as_str(),
            "-e",
            "PROXY_PORT=8000",
            "-e",
            "API_PORT=8001",
            "-e",
            "PROXY_FQDN=shuttleapp.test",
            "-e",
            "SHUTTLE_USERS_TOML=/config/users.toml",
            "-e",
            "SHUTTLE_INITIAL_KEY=ci-test",
            "-e",
            &format!("PROVISIONER_ADDRESS={provisioner_container}"),
            "-v",
            &format!("{}:/config/users.toml", users_toml_file),
            &api_image,
        ]);
        api.api_container = Some(api_container);

        spawn_and_log(&mut run, api_target, api.color);

        api.wait_ready(Duration::from_secs(120));

        api
    }

    fn build_image(service: &str, target: &str, id: &str) -> String {
        let image = format!("shuttle_{service}_{target}_{id}");
        let containerfile = format!("./{service}/Containerfile.dev");

        let mut build = Command::new("docker");

        build
            .args(["build", "-f", &containerfile, "-t", &image, "."])
            .current_dir("../");

        spawn_and_log(&mut build, target, Color::White)
            .wait()
            .ensure_success("failed to build `{service}` image");

        image
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
        let client_target = format!("     {} client", self.target);

        let mut build = Command::new("cargo");
        build
            .args(["build", "--bin", "cargo-shuttle"])
            .current_dir("../");
        spawn_and_log(&mut build, client_target.as_str(), Color::White)
            .wait()
            .ensure_success("failed to build `cargo-shuttle`");

        let mut run = Command::new("../../../target/debug/cargo-shuttle");
        run.args(args)
            .current_dir(path)
            .env("SHUTTLE_API", format!("http://{}", self.api_addr));
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

    #[allow(dead_code)]
    pub fn post(&self, sub_path: &str) -> RequestBuilder {
        reqwest::blocking::Client::new().post(format!("http://{}/{}", self.proxy_addr, sub_path))
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        if let Some(container) = &self.api_container {
            Command::new("docker")
                .args(["stop", container])
                .output()
                .expect("failed to stop api container");
            Command::new("docker")
                .args(["rm", container])
                .output()
                .expect("failed to remove api container");
        }

        if let Some(image) = &self.api_image {
            Command::new("docker")
                .args(["rmi", image])
                .output()
                .expect("failed to remove api image");
        }

        if let Some(container) = &self.provisioner_container {
            Command::new("docker")
                .args(["stop", container])
                .output()
                .expect("failed to stop api container");
            Command::new("docker")
                .args(["rm", container])
                .output()
                .expect("failed to remove api container");
        }

        if let Some(image) = &self.provisioner_image {
            Command::new("docker")
                .args(["rmi", image])
                .output()
                .expect("failed to remove api image");
        }
    }
}
