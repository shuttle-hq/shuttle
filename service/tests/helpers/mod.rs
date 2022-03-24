use std::{
    io::{BufRead, BufReader, Read},
    process::{Command, Stdio},
};

use portpicker::pick_unused_port;

pub struct PostgresInstance {
    port: u16,
    container: String,
    password: String,
}

impl PostgresInstance {
    pub fn new() -> Self {
        let port = pick_unused_port().expect("could not find a free port for postgres");
        let container = "postgres-shuttle-service-integration-test".to_string();
        let password = "password".to_string();

        let mut command = Command::new("docker");
        command.args([
            "run",
            "--name",
            &container,
            "-e",
            &format!("POSTGRES_PASSWORD={}", password),
            "-p",
            &format!("{}:5432", port),
            "postgres:11", // Our Dockerfile image is based on buster which has postgres version 11
        ]);

        Self::wait_for_up(&mut command);

        Self {
            port,
            container,
            password,
        }
    }

    pub fn get_uri(&self) -> String {
        format!(
            "postgres://postgres:{}@localhost:{}",
            self.password, self.port
        )
    }

    fn wait_for_up(command: &mut Command) {
        let mut child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn a postgres container");
        let mut stdout = child
            .stdout
            .take()
            .expect("failed to get stdout of container");

        let mut buf = [0; 2 << 12];
        let mut current_pos = 0;

        'wait: while let Ok(len) = stdout.read(&mut buf[current_pos..]) {
            if len == 0 {
                break;
            }

            current_pos += len;

            if buf[current_pos - 1] != b'\n' {
                continue;
            }

            for line in BufReader::new(&buf[..current_pos]).lines() {
                let line = line.unwrap();
                println!("{}", line);

                if line.contains("PostgreSQL init process complete") {
                    break 'wait;
                }
            }

            current_pos = 0;
        }

        // Rust is just a little too fast for the docker runtime :(
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

impl Drop for PostgresInstance {
    fn drop(&mut self) {
        Command::new("docker")
            .args(["stop", &self.container])
            .spawn()
            .expect("failed to spawn stop for postgres container")
            .wait()
            .expect("postgres container stop failed");

        Command::new("docker")
            .args(["rm", &self.container])
            .spawn()
            .expect("failed to spawn stop for remove container")
            .wait()
            .expect("postgres container remove failed");
    }
}
