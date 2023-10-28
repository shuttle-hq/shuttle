use std::path::{Path, PathBuf};

use shuttle_service::builder::{build_workspace, BuiltService};

#[tokio::test]
#[should_panic(expected = "Build failed. Is the Shuttle runtime missing?")]
async fn not_shuttle() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/not-shuttle", env!("CARGO_MANIFEST_DIR"));
    build_workspace(Path::new(&project_path), false, tx, false)
        .await
        .unwrap();
}

#[tokio::test]
#[should_panic(expected = "Your Shuttle project must be a binary.")]
async fn not_bin() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/not-bin", env!("CARGO_MANIFEST_DIR"));
    match build_workspace(Path::new(&project_path), false, tx, false).await {
        Ok(_) => {}
        Err(e) => panic!("{}", e.to_string()),
    }
}

#[tokio::test]
async fn is_bin() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/is-bin", env!("CARGO_MANIFEST_DIR"));

    assert_eq!(
        build_workspace(Path::new(&project_path), false, tx, false)
            .await
            .unwrap(),
        vec![BuiltService {
            workspace_path: PathBuf::from(&project_path),
            manifest_path: PathBuf::from(&project_path).join("Cargo.toml"),
            package_name: "is-bin".to_string(),
            executable_path: PathBuf::from(&project_path).join("target/debug/is-bin"),
            is_wasm: false,
        }]
    );
}

#[tokio::test]
#[should_panic(expected = "failed to read the Shuttle project manifest")]
async fn not_found() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!(
        "{}/tests/resources/non-existing",
        env!("CARGO_MANIFEST_DIR")
    );
    build_workspace(Path::new(&project_path), false, tx, false)
        .await
        .unwrap();
}

// Test that alpha and next projects are compiled correctly. Any shared library crates should not be compiled too
#[tokio::test]
async fn workspace() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/workspace", env!("CARGO_MANIFEST_DIR"));

    assert_eq!(
        build_workspace(Path::new(&project_path), false, tx, false)
            .await
            .unwrap(),
        vec![
            BuiltService {
                workspace_path: PathBuf::from(&project_path),
                manifest_path: PathBuf::from(&project_path).join("alpha/Cargo.toml"),
                package_name: "alpha".to_string(),
                executable_path: PathBuf::from(&project_path).join("target/debug/alpha"),
                is_wasm: false,
            },
            BuiltService {
                workspace_path: PathBuf::from(&project_path),
                manifest_path: PathBuf::from(&project_path).join("next/Cargo.toml"),
                package_name: "next".to_string(),
                executable_path: PathBuf::from(&project_path)
                    .join("target/wasm32-wasi/debug/next.wasm"),
                is_wasm: true,
            },
        ]
    );
}
