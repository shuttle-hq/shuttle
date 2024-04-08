use portpicker::pick_unused_port;
use std::{
    process::Command,
    thread::sleep,
    time::{Duration, SystemTime},
};

pub struct DockerInstance {
    container_name: String,
    pub uri: String,
}

impl DockerInstance {
    pub fn new(name: &str, api_url: &str, api_key: &str) -> Self {
        let container_name = format!("shuttle_test_permit_{}", name);
        let e1 = format!("PDP_CONTROL_PLANE={api_url}");
        let e2 = format!("PDP_API_KEY={api_key}");
        let e3 = "PDP_OPA_CLIENT_QUERY_TIMEOUT=10";
        let env = [e1.as_str(), e2.as_str(), e3];
        let port = "7000";
        let image = "docker.io/permitio/pdp-v2:0.2.37";
        let is_ready_cmd = vec![
            "exec",
            container_name.as_str(),
            "curl",
            "-f",
            "localhost:7000",
        ];
        let host_port = pick_unused_port().unwrap();
        let port_binding = format!("{}:{}", host_port, port);

        let mut args = vec![
            "run",
            "--rm",
            "--name",
            container_name.as_str(),
            "-p",
            &port_binding,
            "-d",
        ];

        args.extend(env.iter().flat_map(|e| ["-e", e]));

        args.push(image);

        Command::new("docker").args(args).spawn().unwrap();

        Self::wait_ready(Duration::from_secs(120), &is_ready_cmd);

        Self {
            container_name,
            uri: format!("http://localhost:{host_port}"),
        }
    }

    fn wait_ready(mut timeout: Duration, is_ready_cmd: &[&str]) {
        let mut now = SystemTime::now();
        while !timeout.is_zero() {
            let status = Command::new("docker")
                .args(is_ready_cmd)
                .output()
                .unwrap()
                .status;

            if status.success() {
                println!("{is_ready_cmd:?} succeeded...");
                sleep(Duration::from_millis(350));
                return;
            }

            println!("{is_ready_cmd:?} did not succeed this time...");
            sleep(Duration::from_millis(350));

            timeout = timeout
                .checked_sub(now.elapsed().unwrap())
                .unwrap_or_default();
            now = SystemTime::now();
        }
        panic!("timed out while waiting for provisioner DB to come up");
    }

    // Remove the docker container.
    pub fn cleanup(&self) {
        Command::new("docker")
            .args(["stop", self.container_name.as_str()])
            .output()
            .unwrap();
        Command::new("docker")
            .args(["rm", self.container_name.as_str()])
            .output()
            .unwrap();
    }
}
