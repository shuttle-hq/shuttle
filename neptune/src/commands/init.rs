use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use cargo_shuttle::args::parse_and_create_path;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};

use crate::{args::InitArgs, ui::AiUi, Neptune, NeptuneCommandOutput};
use tokio::process::Command;

impl Neptune {
    pub async fn init(&self, args: InitArgs) -> Result<NeptuneCommandOutput> {
        // Persona UI
        let ui = AiUi::new(&self.global_args.output_mode, self.global_args.verbose);
        ui.header("Init");

        let theme = ColorfulTheme::default();
        let no_git = args.no_git;

        // Project name
        let project_name: String = Input::with_theme(&theme)
            .with_prompt("Project name")
            .interact()?;

        // If --from was provided, honor it and skip interactive template choice
        if let Some(git_template) = args.git_template()? {
            // Determine target directory
            let target_dir = if !args.path_provided_arg {
                let default_path = args.path.join(&project_name);
                loop {
                    eprintln!("Where should we create this project?");
                    let directory_str: String = Input::with_theme(&theme)
                        .with_prompt("Directory")
                        .default(format!("{}", default_path.display()))
                        .interact()?;
                    eprintln!();
                    let path = parse_and_create_path(OsString::from(directory_str))?;
                    if fs::read_dir(&path)
                        .expect("init dir to exist and list entries")
                        .count()
                        > 0
                        && !Confirm::with_theme(&theme)
                            .with_prompt("Target directory is not empty. Are you sure?")
                            .default(true)
                            .interact()?
                    {
                        eprintln!();
                        continue;
                    }
                    break path;
                }
            } else {
                args.path.clone()
            };

            ui.step(
                "",
                format!("Cloning template into {}", target_dir.display()),
            );
            self.clone_repo_to_dir_with_subfolder(
                &git_template.auto_path,
                git_template.subfolder.as_deref(),
                &target_dir,
                no_git,
            )
            .await?;
            self.write_project_metadata(&target_dir, &project_name)
                .await?;
            self.generate_agents(&target_dir).await?;

            eprintln!();
            ui.success("✅ Project initialized");
            ui.step("", format!("Path: {}", target_dir.display()));
            if std::env::current_dir().is_ok_and(|d| d != target_dir) {
                eprintln!("You can `cd` to the directory, then:");
            }
            eprintln!("Run `neptune deploy` to deploy it.");
            return Ok(NeptuneCommandOutput::None);
        }

        // Choose between current directory or a starter template
        let choices = vec!["Current working directory", "Choose from templates"];
        let choice = Select::with_theme(&theme)
            .with_prompt("Where do you want to initialize?")
            .items(&choices)
            .default(0)
            .interact()?;

        match choice {
            0 => {
                let cwd = std::env::current_dir()?;
                ui.step("", format!("Initializing in {}", cwd.display()));
                self.write_project_metadata(&cwd, &project_name).await?;
                self.generate_agents(&cwd).await?;
                eprintln!();
                ui.success("✅ Project initialized in current directory");
                eprintln!("Run `neptune deploy` to deploy it.");
            }
            _ => {
                // Template list
                let templates = crate::templates::templates();
                let items: Vec<String> = templates
                    .iter()
                    .map(|t| format!("{}  ({})", t.name, t.url))
                    .collect();
                let idx = Select::with_theme(&theme)
                    .with_prompt("Choose a template")
                    .items(&items)
                    .default(0)
                    .interact()?;
                let selected = &templates[idx];

                // Compute target directory under a base path using the project name
                let base_dir = if args.path_provided_arg {
                    args.path.clone()
                } else {
                    std::env::current_dir()?
                };
                let target_dir: PathBuf = base_dir.join(&project_name);

                // Confirm if path exists and is non-empty
                if target_dir.exists()
                    && fs::read_dir(&target_dir)
                        .ok()
                        .is_some_and(|mut r| r.next().is_some())
                    && !Confirm::with_theme(&theme)
                        .with_prompt("Target directory is not empty. Are you sure?")
                        .default(true)
                        .interact()?
                {
                    eprintln!("Aborted.");
                    return Ok(NeptuneCommandOutput::None);
                }

                ui.step(
                    "",
                    format!(
                        "Cloning template '{}' into {}",
                        selected.name,
                        target_dir.display()
                    ),
                );
                self.clone_repo_to_dir_with_subfolder(selected.url, None, &target_dir, no_git)
                    .await?;
                self.write_project_metadata(&target_dir, &project_name)
                    .await?;
                self.generate_agents(&target_dir).await?;

                eprintln!();
                ui.success("✅ Project initialized");
                ui.step("", format!("Path: {}", target_dir.display()));
                if std::env::current_dir().is_ok_and(|d| d != target_dir) {
                    eprintln!("You can `cd` to the directory, then:");
                }
                eprintln!("Run `neptune deploy` to deploy it.");
            }
        }

        Ok(NeptuneCommandOutput::None)
    }
}

impl Neptune {
    async fn clone_repo_to_dir(&self, url: &str, dest: &Path, remove_git: bool) -> Result<()> {
        if !dest.exists() {
            fs::create_dir_all(dest)?;
        }
        let status = Command::new("git")
            .args(["clone", "--depth", "1", url, &dest.to_string_lossy()])
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("git clone failed for {}", url);
        }
        if remove_git {
            let git_dir = dest.join(".git");
            if git_dir.exists() {
                fs::remove_dir_all(git_dir)?;
            }
        }
        Ok(())
    }

    async fn clone_repo_to_dir_with_subfolder(
        &self,
        url: &str,
        subfolder: Option<&str>,
        dest: &Path,
        remove_git: bool,
    ) -> Result<()> {
        if subfolder.is_none() {
            return self.clone_repo_to_dir(url, dest, remove_git).await;
        }
        // Create a temp dir next to destination
        let parent = dest.parent().unwrap_or_else(|| Path::new("."));
        let tmp = parent.join(format!(
            ".neptune-init-tmp-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));
        if tmp.exists() {
            fs::remove_dir_all(&tmp).ok();
        }
        fs::create_dir_all(&tmp)?;
        // Clone repo to tmp
        let status = Command::new("git")
            .args(["clone", "--depth", "1", url, &tmp.to_string_lossy()])
            .status()
            .await?;
        if !status.success() {
            anyhow::bail!("git clone failed for {}", url);
        }
        // Copy subfolder contents
        let src = tmp.join(subfolder.unwrap());
        if !src.exists() || !src.is_dir() {
            fs::remove_dir_all(&tmp).ok();
            anyhow::bail!("subfolder '{}' not found in repository", subfolder.unwrap());
        }
        if !dest.exists() {
            fs::create_dir_all(dest)?;
        }
        self.copy_dir_recursive(&src, dest, /*ignore_git=*/ true)?;
        // Cleanup
        fs::remove_dir_all(&tmp).ok();
        if remove_git {
            let git_dir = dest.join(".git");
            if git_dir.exists() {
                fs::remove_dir_all(git_dir)?;
            }
        }
        Ok(())
    }

    fn copy_dir_recursive(&self, src: &Path, dest: &Path, ignore_git: bool) -> Result<()> {
        if !dest.exists() {
            fs::create_dir_all(dest)?;
        }
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let name = entry.file_name();
            if ignore_git && name.to_string_lossy() == ".git" {
                continue;
            }
            let from = entry.path();
            let to = dest.join(&name);
            if file_type.is_dir() {
                self.copy_dir_recursive(&from, &to, ignore_git)?;
            } else if file_type.is_file() {
                if to.exists() {
                    // do not overwrite
                    continue;
                }
                fs::copy(&from, &to)?;
            }
        }
        Ok(())
    }

    async fn write_project_metadata(&self, dir: &Path, name: &str) -> Result<()> {
        let meta_dir = dir.join(".neptune");
        if !meta_dir.exists() {
            fs::create_dir_all(&meta_dir)?;
        }
        let project_name_file = meta_dir.join("project_name");
        tokio::fs::write(project_name_file, name.as_bytes()).await?;
        Ok(())
    }
}
