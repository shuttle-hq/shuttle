use std::{path::PathBuf, process::Command};

// This test will compile the `.proto` files and put the generated code
// in `src/generated`. We commit the generated code, and run this test in
// CI to make sure that the generated files are up to date with any changes
// to the `.proto` files.
#[test]
fn bootstrap() {
    let proto_files = &["provisioner.proto", "runtime.proto"];

    let out_dir = PathBuf::from(std::env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("generated");

    tonic_build::configure()
        .out_dir(format!("{}", out_dir.display()))
        .compile(proto_files, &["./"])
        .unwrap();

    let status = Command::new("git")
        .arg("diff")
        .arg("--exit-code")
        .arg("--")
        .arg(format!("{}", out_dir.display()))
        .status()
        .unwrap();

    if !status.success() {
        panic!("You should commit the protobuf files");
    }
}

// This solution is based on this tonic-health pull request:
// https://github.com/hyperium/tonic/pull/1065/
