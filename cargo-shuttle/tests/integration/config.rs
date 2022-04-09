use std::{path::PathBuf, str::FromStr};

use cargo_shuttle::{args::ProjectArgs, config::RequestContext};
use shuttle_common::project::ProjectName;

fn path_from_workspace_root(path: &str) -> PathBuf {
    PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("..")
        .join(path)
}

#[test]
fn get_local_config_finds_name_in_shuttle_toml() {
    let project_args = ProjectArgs {
        working_directory: path_from_workspace_root("examples/axum/hello-world/"),
        name: None,
    };

    let local_config = RequestContext::get_local_config(&project_args).unwrap();

    // FIXME: make a helper for this?
    let name = local_config
        .as_ref()
        .unwrap()
        .name
        .as_ref()
        .unwrap()
        .to_string();
    assert_eq!(name, "hello-world-axum-app");
}

#[test]
fn fixme_running_in_src_subdir_finds_crate_but_fails_to_find_config() {
    let project_args = ProjectArgs {
        working_directory: path_from_workspace_root("examples/axum/hello-world/src"),
        name: None,
    };

    let local_config = RequestContext::get_local_config(&project_args).unwrap();

    // FIXME: make a helper for this?
    let name = local_config
        .as_ref()
        .unwrap()
        .name
        .as_ref()
        .unwrap()
        .to_string();

    // FIXME: this is not the intended behaviour. We should fix this.
    // This should really be "hello-world-axum-app", as above.
    assert_eq!(name, "hello-world");
}

#[test]
fn setting_name_overrides_name_in_config() {
    let project_args = ProjectArgs {
        working_directory: path_from_workspace_root("examples/axum/hello-world/"),
        name: Some(ProjectName::from_str("my-fancy-project-name").unwrap()),
    };

    let local_config = RequestContext::get_local_config(&project_args).unwrap();

    // FIXME: make a helper for this?
    let name = local_config
        .as_ref()
        .unwrap()
        .name
        .as_ref()
        .unwrap()
        .to_string();
    assert_eq!(name, "my-fancy-project-name");
}
