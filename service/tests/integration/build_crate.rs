use std::path::{Path, PathBuf};

use shuttle_service::loader::{build_crate, Runtime};

#[tokio::test(flavor = "multi_thread")]
async fn not_shuttle() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/not-shuttle", env!("CARGO_MANIFEST_DIR"));
    let so_path = match build_crate(Default::default(), Path::new(&project_path), false, tx)
        .await
        .unwrap()
    {
        Runtime::Legacy(path) => path,
        _ => unreachable!(),
    };

    assert!(
        so_path
            .display()
            .to_string()
            .ends_with("tests/resources/not-shuttle/target/debug/libnot_shuttle.so"),
        "did not get expected so_path: {}",
        so_path.display()
    );
}

#[tokio::test]
#[should_panic(
    expected = "Your Shuttle project must be a library. Please add `[lib]` to your Cargo.toml file."
)]
async fn not_lib() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/not-lib", env!("CARGO_MANIFEST_DIR"));
    build_crate(Default::default(), Path::new(&project_path), false, tx)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn not_cdylib() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/not-cdylib", env!("CARGO_MANIFEST_DIR"));
    assert!(matches!(
        build_crate(Default::default(), Path::new(&project_path), false, tx).await,
        Ok(Runtime::Legacy(_))
    ));
    assert!(PathBuf::from(project_path)
        .join("target/debug/libnot_cdylib.so")
        .exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn is_cdylib() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!("{}/tests/resources/is-cdylib", env!("CARGO_MANIFEST_DIR"));
    assert!(matches!(
        build_crate(Default::default(), Path::new(&project_path), false, tx).await,
        Ok(Runtime::Legacy(_))
    ));
    assert!(PathBuf::from(project_path)
        .join("target/debug/libis_cdylib.so")
        .exists());
}

#[tokio::test]
#[should_panic(expected = "failed to read")]
async fn not_found() {
    let (tx, _) = crossbeam_channel::unbounded();
    let project_path = format!(
        "{}/tests/resources/non-existing",
        env!("CARGO_MANIFEST_DIR")
    );
    build_crate(Default::default(), Path::new(&project_path), false, tx)
        .await
        .unwrap();
}
