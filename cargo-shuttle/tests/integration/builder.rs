use std::path::{Path, PathBuf};

use cargo_shuttle::builder::{build_workspace, BuiltService};

#[tokio::test]
#[should_panic(
    expected = "Expected at least one target that Shuttle can build. Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`."
)]
async fn not_shuttle() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/not-shuttle");
    build_workspace(Path::new(&project_path), false, tx)
        .await
        .unwrap();
}

#[tokio::test]
#[should_panic(
    expected = "Expected at least one target that Shuttle can build. Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`."
)]
async fn not_bin() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/not-bin");
    build_workspace(Path::new(&project_path), false, tx)
        .await
        .unwrap();
}

#[tokio::test]
#[should_panic(
    expected = "Expected at least one target that Shuttle can build. Make sure your crate has a binary target that uses a fully qualified `#[shuttle_runtime::main]`."
)]
async fn not_full_macro() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/resources/not-full-macro"
    );
    build_workspace(Path::new(&project_path), false, tx)
        .await
        .unwrap();
}

#[tokio::test]
async fn is_bin() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/is-bin");

    assert_eq!(
        build_workspace(Path::new(&project_path), false, tx)
            .await
            .unwrap(),
        BuiltService {
            workspace_path: PathBuf::from(&project_path),
            target_name: "is-bin".to_string(),
            executable_path: PathBuf::from(&project_path).join("target/debug/is-bin"),
        }
    );
}

#[tokio::test]
async fn is_bin2() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/is-bin2");

    assert_eq!(
        build_workspace(Path::new(&project_path), false, tx)
            .await
            .unwrap(),
        BuiltService {
            workspace_path: PathBuf::from(&project_path),
            target_name: "weirdchamp".to_string(),
            executable_path: PathBuf::from(&project_path).join("target/debug/weirdchamp"),
        }
    );
}

#[tokio::test]
#[should_panic(expected = "Cargo manifest file not found")]
async fn not_found() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = format!(
        "{}/tests/resources/non-existing",
        env!("CARGO_MANIFEST_DIR")
    );
    build_workspace(Path::new(&project_path), false, tx)
        .await
        .unwrap();
}

// Test that workspace projects are compiled correctly
#[tokio::test]
async fn workspace() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);
    tokio::spawn(async move {
        while let Some(l) = rx.recv().await {
            println!("{l}");
        }
    });
    let project_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/resources/workspace");

    assert_eq!(
        build_workspace(Path::new(&project_path), false, tx)
            .await
            .unwrap(),
        BuiltService {
            workspace_path: PathBuf::from(&project_path),
            target_name: "alpha".to_string(),
            executable_path: PathBuf::from(&project_path).join("target/debug/alpha"),
        }
    );
}
