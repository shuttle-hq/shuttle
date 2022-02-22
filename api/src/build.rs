use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::Result;
use cargo::core::compiler::{CompileMode};
use cargo::core::Workspace;
use cargo::ops::CompileOptions;
use rocket::{Data, tokio};
use rocket::data::ByteUnit;

const FS_ROOT: &'static str = "/tmp/crates/";

pub struct ProjectConfig {
    pub name: String,
}

pub(crate) struct Build {
    pub(crate) so_path: PathBuf,
}

#[async_trait]
pub(crate) trait BuildSystem: Send + Sync {
    async fn build<'a>(&self,
             crate_file: Data<'a>,
             project_config: &ProjectConfig) -> Result<Build>;
}

/// A basic build system that uses the file system for caching and storage
pub(crate) struct FsBuildSystem;

#[async_trait]
impl BuildSystem for FsBuildSystem {
    async fn build<'a>(&self, crate_file: Data<'a>, project_config: &ProjectConfig) -> Result<Build> {
        let project_name = &project_config.name;

        // project path
        let project_path = project_path(project_name)?;
        dbg!(&project_path);

        // clear directory
        clear_project_dir(&project_path)?;

        // crate path
        let crate_path = crate_location(&project_path, project_name);
        dbg!(&crate_path);

        // create target file
        let target_file = tokio::fs::File::create(&crate_path).await?;

        // stream to file
        crate_file.open(ByteUnit::max_value()).stream_to(target_file).await?;

        // extract tarball
        extract_tarball(&crate_path, &project_path)?;

        // run cargo build (--debug for now)
        let so_path = build_crate(&project_path)?;

        Ok(Build { so_path })
    }
}

/// Given an api key and project name returns a `PathBuf` to the project
/// If the directory does not exist, creates it.
fn project_path(project: &str) -> Result<PathBuf> {
    let mut project_path = PathBuf::from(FS_ROOT);
    project_path.push(project);
    // create directory
    std::fs::create_dir_all(&project_path)?;
    Ok(project_path)
}

/// Clear everything which is not the target folder from the project path
fn clear_project_dir(project_path: &Path) -> Result<()> {
    // remove everything except for the target folder
    std::fs::read_dir(project_path)?
        .into_iter()
        .map(|dir| dir.unwrap())
        .filter(|dir| dir.file_name() != "target")
        .for_each(|dir| match dir.file_type() {
            Ok(file) => {
                dbg!(&dir);
                if file.is_dir() {
                    std::fs::remove_dir_all(&dir.path()).unwrap();
                } else if file.is_file() {
                    std::fs::remove_file(&dir.path()).unwrap();
                } else if file.is_symlink() {
                    // there shouldn't be any symlinks here
                    unimplemented!()
                }
            }
            Err(_) => {} // file type could not be read, should not happen
        });
    Ok(())
}

/// Given a project path and a project name, return the location of the .crate file
fn crate_location(project_path: &Path, project_name: &str) -> PathBuf {
    project_path.join(project_name).with_extension("crate")
}

/// Given a .crate file (which is a gzipped tarball), extracts the contents
/// into the project_path
fn extract_tarball(crate_path: &Path, project_path: &Path) -> Result<()> {
    Command::new("tar")
        .arg("-xzvf") // extract
        .arg(crate_path)
        .arg("-C")    // target
        .arg(project_path)
        .arg("--strip-components") // remove top-level directory
        .arg("1")
        .output()?;
    Ok(())
}

/// Given a project directory path, builds the crate
fn build_crate(project_path: &Path) -> Result<PathBuf> {
    // This config needs to be tweaked s.t the
    let config = cargo::util::config::Config::default()?;
    let manifest_path = project_path.join("Cargo.toml");

    let ws = Workspace::new(&manifest_path, &config)?;
    let opts = CompileOptions::new(&config, CompileMode::Build)?;
    let _compilation = cargo::ops::compile(&ws, &opts)?;

    todo!("next step is to figure out how to get the .so file from the compilation output")
}
