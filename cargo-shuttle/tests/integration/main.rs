use assert_cmd::Command;

/// creates a `cargo-shuttle` Command instance with some reasonable defaults set.
fn cargo_shuttle_command() -> Command {
    let mut cmd = Command::cargo_bin("cargo-shuttle").unwrap();
    cmd.env(
        "SHUTTLE_API",
        "network support is intentionally broken in tests",
    );
    cmd
}

#[test]
fn default_prints_usage_to_stderr() {
    let mut cmd = cargo_shuttle_command();
    cmd.assert()
        .stderr(predicates::str::is_match("^cargo-shuttle.*\n\nUSAGE:").unwrap());
}

#[test]
fn help_prints_usage_to_stdout() {
    let mut cmd = cargo_shuttle_command();
    cmd.arg("--help")
        .assert()
        .stdout(predicates::str::is_match("^cargo-shuttle.*\n\nUSAGE:").unwrap());
}

#[test]
fn network_support_is_intentionally_broken_in_tests() {
    let mut cmd = cargo_shuttle_command();
    cmd.arg("status").assert().stderr(predicates::str::contains(
        "builder error: relative URL without a base",
    ));
}

#[test]
fn fails_if_working_directory_does_not_exist() {
    let mut cmd = cargo_shuttle_command();
    cmd.arg("status")
        .arg("--working-directory=/path_that_does_not_exist")
        .assert()
        .stderr(
            predicates::str::contains(r#"error: Invalid value for '--working-directory <working-directory>': could not turn "/path_that_does_not_exist" into a real path: No such file or directory (os error 2)"#),
        );
}

#[test]
fn fails_if_working_directory_not_part_of_cargo_workspace() {
    let mut cmd = cargo_shuttle_command();
    cmd.arg("status")
        .arg("--working-directory=/")
        .assert()
        .stderr(predicates::str::contains(
            r#"error: could not find `Cargo.toml` in `/` or any parent directory"#,
        ));
}
