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
