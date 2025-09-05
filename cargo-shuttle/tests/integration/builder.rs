use std::path::PathBuf;

use cargo_shuttle::builder::{cargo_build, BuiltService};

#[tokio::test]
#[should_panic(
    expected = "Expected at least one target that Shuttle can build. Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`."
)]
async fn not_shuttle() {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/not-shuttle"
    ));

    cargo_build(p, false, true).await.unwrap();
}

#[tokio::test]
#[should_panic(
    expected = "Expected at least one target that Shuttle can build. Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`."
)]
async fn not_bin() {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/not-bin"
    ));

    cargo_build(p, false, true).await.unwrap();
}

#[tokio::test]
#[should_panic(
    expected = "Expected at least one target that Shuttle can build. Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`."
)]
async fn not_full_macro() {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/not-full-macro"
    ));

    cargo_build(p, false, true).await.unwrap();
}

#[tokio::test]
async fn is_bin() {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/is-bin"
    ));

    assert_eq!(
        cargo_build(p.clone(), false, true).await.unwrap(),
        BuiltService {
            workspace_path: p.clone(),
            target_name: "is-bin".to_string(),
            executable_path: p.join("target/debug/is-bin"),
        }
    );
}

#[tokio::test]
async fn is_bin2() {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/is-bin2"
    ));

    assert_eq!(
        cargo_build(p.clone(), false, true).await.unwrap(),
        BuiltService {
            workspace_path: p.clone(),
            target_name: "weirdchamp".to_string(),
            executable_path: p.join("target/debug/weirdchamp"),
        }
    );
}

#[tokio::test]
#[should_panic]
async fn no_existing_folder() {
    let p = PathBuf::from(format!(
        "{}/tests/resources/non-existing-folder",
        env!("CARGO_MANIFEST_DIR")
    ));

    cargo_build(p, false, true).await.unwrap();
}

// Test that workspace projects are compiled correctly
#[tokio::test]
async fn workspace() {
    let p = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/workspace"
    ));

    assert_eq!(
        cargo_build(p.clone(), false, true).await.unwrap(),
        BuiltService {
            workspace_path: p.clone(),
            target_name: "alpha".to_string(),
            executable_path: p.join("target/debug/alpha"),
        }
    );
}
