mod builder;
mod init;
mod run;

#[tokio::test]
async fn fails_if_working_directory_does_not_exist() {
    let bin_path = assert_cmd::cargo::cargo_bin("shuttle");
    let mut command = std::process::Command::new(bin_path);
    command.args(["--wd", "/path_that_does_not_exist", "account"]);
    let mut session = rexpect::session::spawn_command(command, Some(500)).unwrap();

    session.exp_string("invalid value").unwrap();
    session.exp_string(
        "could not turn \"/path_that_does_not_exist\" into a real path: No such file or directory (os error 2)"
    ).unwrap();
}

#[tokio::test]
async fn fails_if_local_project_name_in_root() {
    let bin_path = assert_cmd::cargo::cargo_bin("shuttle");
    let mut command = std::process::Command::new(bin_path);
    command.args(["--wd", "/", "run"]);
    let mut session = rexpect::session::spawn_command(command, Some(500)).unwrap();

    session
        .exp_string("expected workspace path to have name")
        .unwrap();
}

#[tokio::test]
async fn fails_if_no_project_id_found() {
    let bin_path = assert_cmd::cargo::cargo_bin("shuttle");
    let mut command = std::process::Command::new(bin_path);
    command.args(["--api-url", "http://shuttle.invalid", "--wd", "/", "logs"]);
    let mut session = rexpect::session::spawn_command(command, Some(500)).unwrap();

    session.exp_string("error sending request for url").unwrap();
}

#[tokio::test]
async fn requests_explicit_project_id_in_non_cargo_workspace() {
    let bin_path = assert_cmd::cargo::cargo_bin("shuttle");
    let mut command = std::process::Command::new(bin_path);
    command.args([
        "--api-url",
        "http://shuttle.invalid",
        "--wd",
        "/",
        "--id",
        "proj_1",
        "logs",
    ]);
    let mut session = rexpect::session::spawn_command(command, Some(500)).unwrap();

    session.exp_string("error sending request for url (http://shuttle.invalid/projects/proj_1/deployments/current)").unwrap();
}

#[tokio::test]
async fn requests_project_list_in_non_cargo_workspace() {
    let bin_path = assert_cmd::cargo::cargo_bin("shuttle");
    let mut command = std::process::Command::new(bin_path);
    command.args([
        "--api-url",
        "http://shuttle.invalid",
        "--wd",
        "/",
        "--name",
        "banana",
        "logs",
    ]);
    let mut session = rexpect::session::spawn_command(command, Some(500)).unwrap();

    session
        .exp_string("error sending request for url (http://shuttle.invalid/projects)")
        .unwrap();
}
