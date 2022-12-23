use std::{env, process::Command};

fn main() {
    // Build binary for runtime so that it can be embedded in the binary for the cli
    let out_dir = env::var_os("OUT_DIR").unwrap();
    Command::new("cargo")
        .arg("build")
        .arg("--package")
        .arg("shuttle-runtime")
        .arg("--target-dir")
        .arg(out_dir)
        .arg("--release")
        .output()
        .expect("failed to build the shuttle runtime");
}
