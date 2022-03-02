use std::path::PathBuf;
use std::process::Command;

const POSTGRES_INITDB: &str = "/usr/lib/postgresql/12/bin/initdb";
const POSTGRES_DATA_DIRECTORY: &str = "pgdata";

struct DatabaseDeployment {
    state: DatabaseDeploymentState,
}

impl DatabaseDeployment {
    fn advance(&self) {
        match &self.state {
            DatabaseDeploymentState::Queued(queued) => {
                let path = queued.project_path.join(POSTGRES_DATA_DIRECTORY);
                match Command::new(POSTGRES_INITDB).arg(&path).output() {
                    Ok(output) => {
                        println!("STDOUT: {}", String::from_utf8(output.stdout).unwrap());
                        println!("STDERR: {}", String::from_utf8(output.stderr).unwrap());
                    }
                    Err(e) => {
                        println!("failed to initialise Postgres at '{}' due to error; {}", path.display(), e);
                    }
                }
            }
            _ => {}
        }
    }
}

enum DatabaseDeploymentState {
    Queued(QueuedState),
    DatabaseInitialised,
    Deployed,
    Error
}

struct QueuedState {
    project_path: PathBuf,
}

struct DatabaseSystem {}

impl DatabaseSystem {
    fn new() -> Self {
        todo!()
    }

    pub(crate) async fn deploy(project_path: PathBuf) {}
}

#[test]
fn testing123() {
    let d = DatabaseDeployment {
        state: DatabaseDeploymentState::Queued(QueuedState { project_path: PathBuf::from("/tmp/testing123") })
    };
    d.advance();
}
