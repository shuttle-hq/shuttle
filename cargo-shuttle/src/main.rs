mod args;
mod client;
mod config;

use std::fs::{read_to_string, File};
use std::io;
use std::io::Write;
use std::rc::Rc;

use anyhow::{Context, Result};
use args::{LoginArgs, ProjectArgs};
use cargo::core::resolver::CliFeatures;
use cargo::core::Workspace;
use cargo::ops::{NewOptions, PackageOpts, Packages};
use cargo_edit::{find, get_latest_dependency, registry_url};
use futures::future::TryFutureExt;
use structopt::StructOpt;
use toml_edit::{value, Array, Document, Item, Table, Value};

use crate::args::{Args, AuthArgs, Command, DeployArgs, InitArgs};
use crate::config::RequestContext;

#[tokio::main]
async fn main() -> Result<()> {
    Shuttle::new().run(Args::from_args()).await
}

pub struct Shuttle {
    ctx: RequestContext,
}

impl Default for Shuttle {
    fn default() -> Self {
        Self::new()
    }
}

impl Shuttle {
    pub fn new() -> Self {
        let ctx = RequestContext::load_global().unwrap();
        Self { ctx }
    }

    pub async fn run(mut self, args: Args) -> Result<()> {
        if matches!(
            args.cmd,
            Command::Deploy(..) | Command::Delete | Command::Status | Command::Logs
        ) {
            self.load_project(&args.project_args)?;
        }

        self.ctx.set_api_url(args.api_url);

        match args.cmd {
            Command::Deploy(deploy_args) => self.deploy(deploy_args).await,
            Command::Init(init_args) => self.init(init_args).await,
            Command::Status => self.status().await,
            Command::Logs => self.logs().await,
            Command::Delete => self.delete().await,
            Command::Auth(auth_args) => self.auth(auth_args).await,
            Command::Login(login_args) => self.login(login_args).await,
        }
    }

    pub fn load_project(&mut self, project_args: &ProjectArgs) -> Result<()> {
        self.ctx.load_local(project_args)
    }

    async fn login(&mut self, login_args: LoginArgs) -> Result<()> {
        let api_key_str = login_args.api_key.unwrap_or_else(|| {
            let url = "https://shuttle.rs/login";

            let _ = webbrowser::open(url);

            println!("If your browser did not automatically open, go to {url}");
            print!("Enter Api Key: ");

            io::stdout().flush().unwrap();

            let mut input = String::new();

            io::stdin().read_line(&mut input).unwrap();

            input
        });

        let api_key = api_key_str.trim().parse()?;

        self.ctx.set_api_key(api_key)?;

        Ok(())
    }

    async fn auth(&mut self, auth_args: AuthArgs) -> Result<()> {
        let api_key = client::auth(self.ctx.api_url(), auth_args.username)
            .await
            .context("failed to retrieve api key")?;
        self.ctx.set_api_key(api_key)?;
        Ok(())
    }

    async fn delete(&self) -> Result<()> {
        client::delete(
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to delete deployment")
    }

    async fn init(&self, args: InitArgs) -> Result<()> {
        // Interface with cargo to initialize new lib package for shuttle
        let opts = NewOptions::new(None, false, true, args.path.clone(), None, None, None)?;
        let cargo_config = cargo::util::config::Config::default()?;
        let init_result = cargo::ops::init(&opts, &cargo_config)?;
        // Mimick `cargo init` behavior and log status or error to shell
        cargo_config
            .shell()
            .status("Created", format!("{} (shuttle) package", init_result))?;

        // Read Cargo.toml into a `Document`
        let cargo_path = args.path.join("Cargo.toml");
        let mut cargo_doc = read_to_string(cargo_path.clone())?.parse::<Document>()?;

        // Remove empty dependencies table to re-insert after the lib table is inserted
        cargo_doc.remove("dependencies");

        // Insert `crate-type = ["cdylib"]` array into `[lib]` table
        let crate_type_array = Array::from_iter(["cdylib"].into_iter());
        let mut lib_table = Table::new();
        lib_table["crate-type"] = Item::Value(Value::Array(crate_type_array));
        cargo_doc["lib"] = Item::Table(lib_table);

        // Fetch the latest shuttle-service version from crates.io
        let manifest_path = find(&Some(args.path)).unwrap();
        let url = registry_url(manifest_path.as_path(), None).expect("Could not find registry URL");
        let latest_shuttle_service =
            get_latest_dependency("shuttle-service", false, &manifest_path, &Some(url))
                .expect("Could not query the latest version of shuttle-service");
        let shuttle_version = latest_shuttle_service
            .version()
            .expect("No latest shuttle-service version available");

        // Insert shuttle-service to `[dependencies]` table
        let mut dep_table = Table::new();
        dep_table["shuttle-service"]["version"] = value(shuttle_version);
        cargo_doc["dependencies"] = Item::Table(dep_table);

        // Truncate Cargo.toml and write the updated `Document` to it
        let mut cargo_toml = File::create(cargo_path)?;
        cargo_toml.write_all(cargo_doc.to_string().as_bytes())?;

        Ok(())
    }

    async fn status(&self) -> Result<()> {
        client::status(
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to get status of deployment")
    }

    async fn logs(&self) -> Result<()> {
        client::logs(
            self.ctx.api_url(),
            self.ctx.api_key()?,
            self.ctx.project_name(),
        )
        .await
        .context("failed to get logs of deployment")
    }

    async fn deploy(&self, args: DeployArgs) -> Result<()> {
        let package_file = self
            .run_cargo_package(args.allow_dirty)
            .context("failed to package cargo project")?;

        let key = self.ctx.api_key()?;

        client::deploy(
            package_file,
            self.ctx.api_url(),
            key,
            self.ctx.project_name(),
        )
        .and_then(|_| {
            client::secrets(
                self.ctx.api_url(),
                key,
                self.ctx.project_name(),
                self.ctx.secrets(),
            )
        })
        .await
        .context("failed to deploy cargo project")
    }

    // Packages the cargo project and returns a File to that file
    fn run_cargo_package(&self, allow_dirty: bool) -> Result<File> {
        let config = cargo::util::config::Config::default()?;

        let working_directory = self.ctx.working_directory();
        let path = working_directory.join("Cargo.toml");

        let ws = Workspace::new(&path, &config)?;
        let opts = PackageOpts {
            config: &config,
            list: false,
            check_metadata: true,
            allow_dirty,
            verify: false,
            jobs: None,
            to_package: Packages::Default,
            targets: vec![],
            cli_features: CliFeatures {
                features: Rc::new(Default::default()),
                all_features: false,
                uses_default_features: true,
            },
        };

        let locks = cargo::ops::package(&ws, &opts)?.expect("unwrap ok here");
        let owned = locks.get(0).unwrap().file().try_clone()?;
        Ok(owned)
    }
}
