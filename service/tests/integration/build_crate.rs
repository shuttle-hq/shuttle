use std::path::{Path, PathBuf};

use shuttle_service::builder::{build_workspace, BuiltService};

#[tokio::test]
#[should_panic(expected = "Build failed. Is the Shuttle runtime missing?")]
async fn not_shuttle() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = format!("{}/tests/resources/not-shuttle", env!("CARGO_MANIFEST_DIR"));
    build_workspace(Path::new(&project_path), false, tx, false)
        .await
        .unwrap();
}

#[tokio::test]
#[should_panic(
    expected = "Did not find any packages that Shuttle can run. Make sure your crate has a binary target that uses `#[shuttle_runtime::main]`."
)]
async fn not_bin() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
    let project_path = format!("{}/tests/resources/not-bin", env!("CARGO_MANIFEST_DIR"));
    match build_workspace(Path::new(&project_path), false, tx, false).await {
        Ok(_) => {}
        Err(e) => panic!("{}", e.to_string()),
    }
}

#[tokio::test]
async fn is_bin() {
    let (tx, _) = tokio::sync::mpsc::channel::<String>(256);
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
        }]
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
    build_workspace(Path::new(&project_path), false, tx, false)
        .await
        .unwrap();
}

// Test that alpha projects are compiled correctly. Any shared library crates should not be compiled too
#[tokio::test]
async fn workspace() {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(256);
    tokio::spawn(async move {
        while let Some(l) = rx.recv().await {
            println!("{l}");
        }
    });
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
            },
            BuiltService {
                workspace_path: PathBuf::from(&project_path),
                manifest_path: PathBuf::from(&project_path).join("alpha2/Cargo.toml"),
                package_name: "alpha2".to_string(),
                executable_path: PathBuf::from(&project_path).join("target/debug/alpha2"),
            },
        ]
    );
}
