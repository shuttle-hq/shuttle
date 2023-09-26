use std::fs::{self, read_to_string};
use std::{
    fmt::Write,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use git2::Repository;
use regex::Regex;
use shuttle_common::project::ProjectName;
use tempfile::tempdir;
use toml_edit::{value, Document};
use url::Url;

use crate::args::TemplateLocation;

pub fn generate_project(
    dest: PathBuf,
    name: &ProjectName,
    temp_loc: TemplateLocation,
) -> Result<()> {
    let gen = GenerateFrom::from(temp_loc).context("Failed to parse template location")?;

    println!(r#"Creating project "{name}" in "{}""#, dest.display());

    gen.generate(&dest, |path: &Path| {
        // Modifications applied to the content of the template:
        set_crate_name(path, name.as_str())
            .with_context(|| "Failed to set crate name. No Cargo.toml in template?")?;
        edit_shuttle_toml(path).with_context(|| "Failed to edit Shuttle.toml.")?;
        create_gitignore_file(path).with_context(|| "Failed to create .gitignore file.")?;
        Ok(())
    })?;

    // Initialize a Git repository in the destination directory if there
    // is no existing Git repository present in the surrounding folders.
    let missing_git_repo = Repository::discover(&dest).is_err();
    if missing_git_repo {
        Repository::init(&dest).context("Failed to initialize project repository")?;
    }

    Ok(())
}

/// Location where the template to generate a project is found.
#[derive(Debug, Clone)]
enum GenerateFrom {
    /// `LocalPath` also supports sub-folders, but it doesn't need
    /// multiple entries for this. The primary path and the sub-folder
    /// path are simply concatenated when the `LocalPath` is constructed.
    LocalPath(PathBuf),
    Url {
        url: String,                // e.g. `https://github.com/shuttle-hq/shuttle
        subfolder: Option<PathBuf>, // e.g. `cargo-shuttle`
    },
    RemoteRepo {
        vendor: GitVendor,
        owner: String,              // e.g. `shuttle-hq`
        name: String,               // e.g. `shuttle`
        subfolder: Option<PathBuf>, // e.g. `cargo-shuttle`
    },
}

impl GenerateFrom {
    // Very loose restrictions are applied to repository names.
    // What's important is that all names that are valid by the vendor's
    // rules are accepted here. There is no need to check that the user
    // actually provided a name that the vendor would accept.
    const GIT_PATTERN: &str = "^(?:(?<vendor>gh|gl|bb):)?(?<owner>[^/.:]+)/(?<name>[^/.:]+)$";

    fn from(loc: TemplateLocation) -> Result<Self> {
        let git_re = Regex::new(Self::GIT_PATTERN).unwrap();

        if let Some(caps) = git_re.captures(&loc.auto_path) {
            let vendor = match caps.name("vendor").map(|v| v.as_str()) {
                Some("gl") => GitVendor::GitLab,
                Some("bb") => GitVendor::BitBucket,
                // GitHub is the default vendor if no other vendor is specified.
                Some("gh") | None => GitVendor::GitHub,
                Some(_) => unreachable!("should never match unknown vendor"),
            };

            Ok(Self::RemoteRepo {
                vendor,
                // `owner` and `name` are required for the regex to
                // match. Thus, we don't need to check if they exist.
                owner: caps["owner"].to_owned(),
                name: caps["name"].to_owned(),
                subfolder: loc.subfolder.map(PathBuf::from),
            })
        } else if Path::new(&loc.auto_path).is_absolute() || loc.auto_path.starts_with('.') {
            // Local paths to template locations must be absolute or start with a
            // dot pattern. This way, conflicts between relative path in the form
            // `foo/bar` and repository name in the form `owner/name` are avoided.
            match loc.subfolder {
                Some(subfolder) => Ok(Self::LocalPath(Path::new(&loc.auto_path).join(subfolder))),
                None => Ok(Self::LocalPath(PathBuf::from(loc.auto_path))),
            }
        } else if let Ok(url) = loc.auto_path.parse::<Url>() {
            if url.scheme() == "http" || url.scheme() == "https" {
                Ok(Self::Url {
                    url: loc.auto_path,
                    subfolder: loc.subfolder.map(PathBuf::from),
                })
            } else {
                println!(
                    "URL scheme is not supported. Please use HTTP of HTTPS for URLs\
		     , or use another method of specifying the template location."
                );
                println!(
                    "HINT: Here you can find examples of how to \
		     select a template: https://github.com/shuttle\
		     -hq/shuttle-examples#how-to-clone-run-and-deploy-an-example"
                );
                anyhow::bail!("invalid URL scheme")
            }
        } else {
            anyhow::bail!("template location is invalid")
        }
    }

    fn generate<F>(self, dest: &Path, prepare: F) -> Result<()>
    where
        F: FnOnce(&Path) -> Result<()>,
    {
        let temp_dir =
            tempdir().context("Failed to create a temporary directory to generate project into")?;

        // `gen` is the path to the directory inside `temp_dir` where the
        // template is generated. It differs from `temp_dir` only if a
        // remote repository is used with a sub-folder.
        let gen = match self {
            Self::RemoteRepo {
                vendor,
                owner,
                name,
                subfolder,
            } => {
                let url = format!("{vendor}{owner}/{name}.git");
                Self::generate_remote_repo(temp_dir.path(), url, subfolder)?
            }
            Self::Url { url, subfolder } => {
                Self::generate_remote_repo(temp_dir.path(), url, subfolder)?
            }
            Self::LocalPath(path) => Self::generate_local_path(temp_dir.path(), path)?,
        };

        // Modify the template.
        prepare(&gen).context("Failed to prepare template")?;

        copy_template(&gen, dest)
            .context("Failed to copy the prepared template to the destination")?;

        temp_dir
            .close()
            .context("Failed to close temporary directory that the project was generated into")?;
        Ok(())
    }

    fn generate_remote_repo(
        temp_path: &Path,
        url: String,
        subfolder: Option<PathBuf>,
    ) -> Result<PathBuf> {
        Repository::clone(&url, temp_path)
            .with_context(|| format!("Failed to clone Git repository at {url}"))?;

        // Extend the path to the directory that's used to generate the template
        // with the path to the specified sub-folder in the cloned repository.
        if let Some(subfolder) = subfolder {
            Ok(temp_path.join(subfolder))
        } else {
            Ok(temp_path.into())
        }
    }

    fn generate_local_path(temp_path: &Path, path: PathBuf) -> Result<PathBuf> {
        if path.exists() {
            copy_template(&path, temp_path)?;
            Ok(temp_path.into())
        } else {
            anyhow::bail!(format!(
                "Template directory {} doesn't exist",
                path.display()
            ))
        }
    }
}

/// Copy everything from `src` to `dest`. The `.git` directory
/// is not copied. The general procedure is the same as the one
/// used in `cargo-generate` (https://github.com/cargo-generate/cargo-generate/blob/073b938b5205678bb25bd05aa8036b96ed5f22a7/src/lib.rs#L450).
fn copy_template(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let entry_type = entry.file_type()?;
        let entry_name = entry.file_name().to_string_lossy().to_string();

        let entry_dest = dest.join(&entry_name);

        if entry_type.is_dir() {
            if entry_name == ".git" {
                continue;
            }

            // Recursion!
            copy_template(&entry.path(), &entry_dest)?;
        } else if entry_type.is_file() {
            if entry_dest.exists() {
                println!(
                    "Error: file '{}' already exists. Cannot overwrite",
                    entry_dest.display()
                );
            } else {
                // Copy this file.
                fs::copy(&entry.path(), &entry_dest)?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Copy, Clone)]
enum GitVendor {
    GitHub,
    BitBucket,
    GitLab,
}

impl GitVendor {
    fn as_str(&self) -> &'static str {
        match self {
            Self::GitHub => "https://github.com/",
            Self::BitBucket => "https://bitbucket.org/",
            Self::GitLab => "https://gitlab.com/",
        }
    }
}

impl std::fmt::Display for GitVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

fn set_crate_name(path: &Path, name: &str) -> Result<()> {
    let path = path.join("Cargo.toml");
    let toml_str = read_to_string(&path)?;
    let mut doc = toml_str.parse::<Document>()?;

    // change the name
    doc["package"]["name"] = value(name);

    // write the file back out
    std::fs::write(&path, doc.to_string())?;

    Ok(())
}

fn edit_shuttle_toml(path: &Path) -> Result<()> {
    let path = path.join("Shuttle.toml");
    if !path.exists() {
        // Do nothing if template has no Shuttle.toml
        return Ok(());
    }
    let toml_str = read_to_string(&path)?;
    let mut doc = toml_str.parse::<Document>()?;

    // The Shuttle.toml project name override will likely already be in use,
    // so that field is not wanted in a newly cloned template.

    // remove the name
    doc.remove("name");

    if doc.len() == 0 {
        // if "name" was the only property in the doc, delete the file
        let _ = std::fs::remove_file(&path);

        return Ok(());
    }

    // write the file back out
    std::fs::write(&path, doc.to_string())?;

    Ok(())
}

/// Adds any missing recommended gitignore rules
fn create_gitignore_file(path: &Path) -> Result<()> {
    let path = path.join(".gitignore");
    let mut contents = std::fs::read_to_string(&path).unwrap_or_default();

    for rule in ["/target", ".shuttle-storage", "Secrets*.toml"] {
        if !contents.lines().any(|l| l == rule) {
            writeln!(&mut contents, "{rule}")?;
        }
    }

    std::fs::write(&path, contents)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_gen_from {
        // Two string literals, the second of which becomes an `Option`.
        ($auto_path:literal, $subfolder:literal) => {
            GenerateFrom::from(TemplateLocation {
                auto_path: $auto_path.to_owned(),
                subfolder: Some($subfolder.to_owned()),
            })
        };

        // A string literal and an expression (`None`).
        ($auto_path:literal, $subfolder:expr) => {
            GenerateFrom::from(TemplateLocation {
                auto_path: $auto_path.to_owned(),
                subfolder: $subfolder,
            })
        };
    }

    #[test]
    fn parse_valid_template_location() {
        test_gen_from!("blah/eww", "foo").unwrap();
        test_gen_from!("blah/eww", "foo/bar").unwrap();
        test_gen_from!("gh:blah/eww", None).unwrap();
        test_gen_from!("gl:blah/eww", None).unwrap();
        test_gen_from!("bb:blah/eww", None).unwrap();
        test_gen_from!("https://github.com/shuttle-hq/shuttle-examples", None).unwrap();
        test_gen_from!(
            "https://github.com/shuttle-hq/shuttle-examples",
            "rocket/hello-world"
        )
        .unwrap();
        test_gen_from!("https://github.com/shuttle-hq/shuttle-examples.git", None).unwrap();
        test_gen_from!(
            "https://github.com/shuttle-hq/shuttle-examples.git",
            "rocket/hello-world"
        )
        .unwrap();
        test_gen_from!("./", None).unwrap();
        test_gen_from!("./foo", None).unwrap();
        test_gen_from!("./foo/bar", None).unwrap();
        test_gen_from!("./foo/bar/baz", None).unwrap();
        test_gen_from!("../foo/bar", None).unwrap();
        test_gen_from!("../foo/bar/baz", None).unwrap();
        test_gen_from!("./foo", "warp/hello-world").unwrap();
        test_gen_from!("./foo/bar", "warp/hello-world").unwrap();
        test_gen_from!("./foo/bar/baz", "warp/hello-world").unwrap();
        test_gen_from!("../foo/bar", "warp/hello-world").unwrap();
        test_gen_from!("../foo/bar/baz", "warp/hello-world").unwrap();
        test_gen_from!("/", None).unwrap();
        test_gen_from!("/foo", None).unwrap();
        test_gen_from!("/foo/bar", None).unwrap();
        test_gen_from!("/foo/bar/baz", None).unwrap();
        test_gen_from!("/foo/bar", None).unwrap();
        test_gen_from!("/foo/bar/baz", None).unwrap();
        test_gen_from!("/foo", "warp/hello-world").unwrap();
        test_gen_from!("/foo/bar", "warp/hello-world").unwrap();
        test_gen_from!("/foo/bar/baz", "warp/hello-world").unwrap();
        test_gen_from!("/foo/bar", "warp/hello-world").unwrap();
        test_gen_from!("/foo/bar/baz", "warp/hello-world").unwrap();

        // Examples from the `shuttle-examples` repo.

        // GitHub prefix. Change to 'gl:' or 'bb:' for GitLab or BitBucket
        // cargo shuttle init --from gh:username/repository
        test_gen_from!("gh:username/repository", None).unwrap();

        // Also GitHub
        // cargo shuttle init --from username/repository
        let gen = test_gen_from!("username/repsoitory", None).unwrap();
        assert!(matches!(
            gen,
            GenerateFrom::RemoteRepo {
                vendor: GitVendor::GitHub,
                ..
            }
        ));

        // From local folder
        test_gen_from!("../path/to/folder", None).unwrap();
        test_gen_from!("/home/user/some/folder", None).unwrap();
    }

    #[test]
    fn parse_invalid_template_location() {
        test_gen_from!("blah/eww/woo", "foo").expect_err("ambiguous path");
        test_gen_from!("blah/eww/woo", "foo/bar").expect_err("ambiguous path");
        test_gen_from!("gh:blah/eww/woo", None).expect_err("ambiguous path");
        test_gen_from!("gl:blah/eww/woo", None).expect_err("ambiguous path");
        test_gen_from!("bb:blah/eww/woo", None).expect_err("ambiguous path");
        test_gen_from!("xy:blah/eww", None).expect_err("unknown vendor");
    }
}
