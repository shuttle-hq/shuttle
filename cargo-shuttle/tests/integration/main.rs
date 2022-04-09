use assert_cmd::Command;

#[test]
fn default_prints_usage_to_stderr() {
    let mut cmd = Command::cargo_bin("cargo-shuttle").unwrap();
    cmd.assert()
        .stderr(predicates::str::is_match("^cargo-shuttle.*\n\nUSAGE:").unwrap());
}

#[test]
fn help_prints_usage_to_stdout() {
    let mut cmd = Command::cargo_bin("cargo-shuttle").unwrap();
    cmd.arg("--help")
        .assert()
        .stdout(predicates::str::is_match("^cargo-shuttle.*\n\nUSAGE:").unwrap());
}
