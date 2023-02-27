use std::io::{self, stderr, stdout, BufRead, Write};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::str;
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use std::{env, path};

use crossterm::style::{Color, Stylize};
use reqwest::blocking::RequestBuilder;

use lazy_static::lazy_static;
use tempfile::{Builder, TempDir};

/// The directory given to `cargo shuttle` run in the context of E2E
/// testing
pub enum TempCargoHome {
    /// A directory managed by the caller, no patch applied
    User(PathBuf),
    /// A directory managed by this crate is created, applies the
    /// patch as required
    Managed(TempDir),
}

impl TempCargoHome {
    /// Initialize a new `TempCargoHome` with a `shuttle-service`
    /// patch unless `SHUTTLE_CARGO_HOME` is set, then use that. With
    /// the latter, no patch is applied
    pub fn from_env_or_new() -> Self {
        match env::var("SHUTTLE_CARGO_HOME") {
            Ok(path) => Self::User(path.into()),
            Err(_) => {
                let dir = Builder::new().prefix("shuttle-tests").tempdir().unwrap();

                // Apply the `patch.crates-io` for `shuttle-service`
                let mut config = std::fs::File::create(dir.path().join("config.toml")).unwrap();
                write!(
                    config,
                    r#"[patch.crates-io]
shuttle-service = {{ path = "{}" }}
shuttle-aws-rds = {{ path = "{}" }}
shuttle-persist = {{ path = "{}" }}
shuttle-shared-db = {{ path = "{}" }}
shuttle-secrets = {{ path = "{}" }}
shuttle-static-folder = {{ path = "{}" }}"#,
                    WORKSPACE_ROOT.join("service").display(),
                    WORKSPACE_ROOT.join("resources").join("aws-rds").display(),
                    WORKSPACE_ROOT.join("resources").join("persist").display(),
                    WORKSPACE_ROOT.join("resources").join("shared-db").display(),
                    WORKSPACE_ROOT.join("resources").join("secrets").display(),
                    WORKSPACE_ROOT
                        .join("resources")
                        .join("static-folder")
                        .display(),
                )
                .unwrap();

                Self::Managed(dir)
            }
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::User(path) => path.as_path(),
            Self::Managed(dir) => dir.path(),
        }
    }

    pub fn display(&self) -> path::Display<'_> {
        self.path().display()
    }
}

lazy_static! {
    static ref WORKSPACE_ROOT: PathBuf = {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    };
    static ref DOCKER: PathBuf = which::which("docker").unwrap();
    static ref MAKE: PathBuf = which::which("make").unwrap();
    static ref CARGO: PathBuf = which::which("cargo").unwrap();
    static ref DB_FQDN: String = env::var("DB_FQDN").unwrap();
    pub static ref APPS_FQDN: String = env::var("APPS_FQDN").unwrap();
    static ref CARGO_HOME: TempCargoHome = TempCargoHome::from_env_or_new();
    static ref LOCAL_UP: () = {
        println!(
            "
----------------------------------- PREPARING ------------------------------------
docker: {}
make: {}
cargo: {}
CARGO_HOME: {}
----------------------------------------------------------------------------------
",
            DOCKER.display(),
            MAKE.display(),
            CARGO.display(),
            CARGO_HOME.display()
        );

        Command::new(MAKE.as_os_str())
            .arg("up")
            .current_dir(WORKSPACE_ROOT.as_path())
            .output()
            .ensure_success("failed to `make up`");

        Command::new(CARGO.as_os_str())
            .args(["build", "--bin", "cargo-shuttle"])
            .current_dir(WORKSPACE_ROOT.as_path())
            .output()
            .ensure_success("failed to `cargo build --bin cargo-shuttle`");

        let admin_key = if let Ok(key) = env::var("SHUTTLE_API_KEY") {
            key
        } else {
            "e2e-test-key".to_string()
        };

        _ = Command::new(DOCKER.as_os_str())
            .args([
                "compose",
                "--file",
                "docker-compose.rendered.yml",
                "--project-name",
                "shuttle-dev",
                "exec",
                "gateway",
                "/usr/local/bin/service",
                "--state=/var/lib/shuttle",
                "init",
                "--name",
                "test",
                "--key",
                &admin_key,
            ])
            .output();
    };
}

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

impl EnsureSuccess for io::Result<Output> {
    fn ensure_success<S: AsRef<str>>(self, s: S) {
        self.map(|output| {
            let _ = stderr().write_all(&output.stderr);
            let _ = stdout().write_all(&output.stdout);
            output.status
        })
        .ensure_success(s)
    }
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
            println!("{} {}", target, line.unwrap());
        }

        current_pos = 0;
    }

    // Log last
    if current_pos != 0 {
        for line in io::BufReader::new(&buf[..current_pos]).lines() {
            println!("{} {}", target, line.unwrap());
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
    let stdout_target = format!("{} >>>", target).with(color);
    let stderr_target = format!("{} >>>", target).bold().with(color);
    std::thread::spawn(move || log_lines(&mut stdout, stdout_target));
    std::thread::spawn(move || log_lines(&mut stderr, stderr_target));
    child
}

pub struct Services {
    api_addr: SocketAddr,
    proxy_addr: SocketAddr,
    /// Path within the examples dir to a specific example
    example_path: String,
    target: String,
    color: Color,
}

impl Services {
    fn new_free<D, C>(target: D, example_path: D, color: C) -> Self
    where
        D: std::fmt::Display,
        C: Into<Color>,
    {
        Self {
            api_addr: "127.0.0.1:8001".parse().unwrap(),
            proxy_addr: "127.0.0.1:8000".parse().unwrap(),
            target: target.to_string(),
            color: color.into(),
            example_path: example_path.to_string(),
        }
    }

    /// Initializes a a test client
    ///
    /// # Arguments
    ///
    /// * `target` - A string that describes the test target
    /// * `example_path` - Path to a specific example within the examples dir, this is where
    ///   `project new` and `deploy` will run
    /// * `color` - a preferably unique `crossterm::style::Color` to distinguish test logs
    pub fn new_docker<D, C>(target: D, example_path: D, color: C) -> Self
    where
        D: std::fmt::Display,
        C: Into<Color>,
    {
        let _ = *LOCAL_UP;
        let service = Self::new_free(target, example_path, color);
        service.wait_ready(Duration::from_secs(15));

        // Make sure provisioner is ready, else deployers will fail to start up
        service.wait_postgres_ready(Duration::from_secs(15));
        service.wait_mongodb_ready(Duration::from_secs(15));
        sleep(Duration::from_secs(5));

        service
    }

    pub fn wait_ready(&self, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            match reqwest::blocking::get(format!("http://{}", self.api_addr)) {
                Ok(resp) if resp.status().is_success() => return,
                _ => sleep(Duration::from_secs(1)),
            }
            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for gateway to / OK");
    }

    pub fn wait_postgres_ready(&self, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let mut run = Command::new(DOCKER.as_os_str());
            run.args([
                "compose",
                "--file",
                "docker-compose.rendered.yml",
                "--project-name",
                "shuttle-dev",
                "exec",
                "postgres",
                "pg_isready",
            ]);

            if spawn_and_log(&mut run, &self.target, self.color)
                .wait()
                .unwrap()
                .success()
            {
                return;
            } else {
                sleep(Duration::from_secs(1));
            }
            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for postgres to be ready");
    }

    pub fn wait_mongodb_ready(&self, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let mut run = Command::new(DOCKER.as_os_str());
            run.args([
                "compose",
                "--file",
                "docker-compose.rendered.yml",
                "--project-name",
                "shuttle-dev",
                "exec",
                "mongodb",
                "mongo",
                "--eval",
                "print(\"accepting connections\")",
            ]);

            if spawn_and_log(&mut run, &self.target, self.color)
                .wait()
                .unwrap()
                .success()
            {
                return;
            } else {
                sleep(Duration::from_secs(1));
            }
            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for mongodb to be ready");
    }

    pub fn wait_deployer_ready(&self, mut timeout: Duration) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let mut run = Command::new(WORKSPACE_ROOT.join("target/debug/cargo-shuttle"));

            if env::var("SHUTTLE_API_KEY").is_err() {
                run.env("SHUTTLE_API_KEY", "e2e-test-key");
            }

            run.env("CARGO_HOME", CARGO_HOME.path());
            run.args(["project", "status"])
                .current_dir(self.get_full_project_path());
            let stdout = run.output().unwrap().stdout;
            let stdout = String::from_utf8(stdout).unwrap();

            if stdout.contains("ready") {
                return;
            } else {
                sleep(Duration::from_secs(1));
            }
            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for deployer to be ready");
    }

    pub fn run_client<'s, I>(&self, args: I) -> Child
    where
        I: IntoIterator<Item = &'s str>,
    {
        let mut run = Command::new(WORKSPACE_ROOT.join("target/debug/cargo-shuttle"));

        if env::var("SHUTTLE_API_KEY").is_err() {
            run.env("SHUTTLE_API_KEY", "e2e-test-key");
        }

        run.env("CARGO_HOME", CARGO_HOME.path());

        run.args(args).current_dir(self.get_full_project_path());
        spawn_and_log(&mut run, &self.target, self.color)
    }

    /// Starts a project and deploys a service for the example in `self.example_path`
    pub fn deploy(&self) {
        self.run_client(["project", "new"])
            .wait()
            .ensure_success("failed to run deploy");

        self.wait_deployer_ready(Duration::from_secs(120));

        self.run_client(["deploy", "--allow-dirty"])
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

    /// Gets the full path: the path within examples to a specific example appended to the workspace root
    pub fn get_full_project_path(&self) -> PathBuf {
        WORKSPACE_ROOT.join("examples").join(&self.example_path)
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        // Initiate project destruction on test completion
        _ = self.run_client(["project", "rm"]).wait();
    }
}
