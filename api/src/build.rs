use anyhow::{anyhow, Context, Result};
use cargo::core::compiler::CompileMode;
use cargo::core::Workspace;
use cargo::ops::CompileOptions;
use lib::ProjectConfig;
use rocket::tokio;
use rocket::tokio::io::AsyncWriteExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(debug_assertions)]
const DEFAULT_FS_ROOT: &'static str = "/tmp/unveil/crates/";

#[cfg(not(debug_assertions))]
// as per: https://stackoverflow.com/questions/1510104/where-to-store-application-data-non-user-specific-on-linux
const DEFAULT_FS_ROOT: &'static str = "/var/lib/unveil/crates/";

pub(crate) struct Build {
    pub(crate) so_path: PathBuf,
}

#[async_trait]
pub(crate) trait BuildSystem: Send + Sync {
    async fn build(
        &self,
        crate_bytes: &Vec<u8>,
        project_config: &ProjectConfig,
        buf: Box<dyn std::io::Write + Send>,
    ) -> Result<Build>;
}

/// A basic build system that uses the file system for caching and storage
pub(crate) struct FsBuildSystem {
    fs_root: PathBuf,
}

impl FsBuildSystem {
    /// Intialises the FS Build System. Optionally you can define the root
    /// of its file system. If unspecified, will default to `FS_ROOT`.
    /// The FS Build System will fail to intialise if the directory does not.
    /// exist
    pub(crate) fn initialise(path: Option<PathBuf>) -> Result<Self> {
        let fs_root = path.unwrap_or(PathBuf::from(DEFAULT_FS_ROOT));
        if !(fs_root.exists()) {
            return Err(anyhow!(r#"
            Failed to initialise FS Build System.
            The path {:?} does not exist.
            Please create the directory to continue with deployment"#, &fs_root));
        }
        Ok(
            FsBuildSystem {
                fs_root
            }
        )
    }

    /// Given an api key and project name returns a `PathBuf` to the project
    /// If the directory does not exist, creates it.
    fn project_path(&self, project: &str) -> Result<PathBuf> {
        let mut project_path = self.fs_root.clone();
        project_path.push(project);
        // create directory
        std::fs::create_dir_all(&project_path)?;
        Ok(project_path)
    }
}

#[async_trait]
impl BuildSystem for FsBuildSystem {
    async fn build(
        &self,
        crate_bytes: &Vec<u8>,
        project_config: &ProjectConfig,
        buf: Box<dyn std::io::Write + Send>,
    ) -> Result<Build> {
        let project_name = &project_config.name;

        // project path
        let project_path = self.project_path(project_name)?;
        dbg!(&project_path);

        // clear directory
        clear_project_dir(&project_path)?;

        // crate path
        let crate_path = crate_location(&project_path, project_name);
        dbg!(&crate_path);

        // create target file
        let mut target_file = tokio::fs::File::create(&crate_path).await?;

        // write bytes to file
        target_file.write_all(crate_bytes).await?;

        // extract tarball
        extract_tarball(&crate_path, &project_path)?;

        // run cargo build (--debug for now)
        let so_path = build_crate(&project_path, buf)?;

        Ok(Build { so_path })
    }
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
fn build_crate(project_path: &Path, buf: Box<dyn std::io::Write>) -> Result<PathBuf> {
    let mut shell = cargo::core::Shell::from_write(buf);
    shell.set_verbosity(cargo::core::Verbosity::Normal);

    let cwd = std::env::current_dir()
        .with_context(|| "couldn't get the current directory of the process")?;
    let homedir = cargo::util::homedir(&cwd).ok_or_else(|| {
        anyhow!(
            "Cargo couldn't find your home directory. \
                 This probably means that $HOME was not set."
        )
    })?;

    let config = cargo::Config::new(shell, cwd, homedir);

    let manifest_path = project_path.join("Cargo.toml");

    let ws = Workspace::new(&manifest_path, &config)?;
    let opts = CompileOptions::new(&config, CompileMode::Build)?;
    let compilation = cargo::ops::compile(&ws, &opts)?;

    if compilation.cdylibs.is_empty() {
        return Err(anyhow!("a cdylib was not created"));
    }

    Ok(compilation.cdylibs[0].path.clone())
}
