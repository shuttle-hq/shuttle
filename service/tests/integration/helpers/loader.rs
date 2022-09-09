use std::path::PathBuf;
use std::process::Command;

use shuttle_service::loader::{Loader, LoaderError};

pub fn build_so_create_loader(resources: &str, crate_name: &str) -> Result<Loader, LoaderError> {
    let crate_dir: PathBuf = [resources, crate_name].iter().collect();

    Command::new("cargo")
        .args(["build", "--release", "--color", "always"])
        .current_dir(&crate_dir)
        .spawn()
        .unwrap()
        .wait()
        .unwrap();

    let dashes_replaced = crate_name.replace('-', "_");

    let lib_name = if cfg!(target_os = "windows") {
        format!("{}.dll", dashes_replaced)
    } else {
        format!("lib{}.so", dashes_replaced)
    };

    let so_path = crate_dir.join("target/release").join(lib_name);

    Loader::from_so_file(&so_path)
}
