use crate::cargo_builder::{CargoBuilder, Dependency};
use anyhow::Result;
use cargo::ops::NewOptions;
use indoc::indoc;
use std::collections::HashMap;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use toml_edit::{value, Array, Document, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::Display, strum::EnumIter)]
#[strum(serialize_all = "kebab-case")]
pub enum Framework {
    ActixWeb,
    Axum,
    Rocket,
    Tide,
    Tower,
    Poem,
    Salvo,
    Serenity,
    Poise,
    Warp,
    Thruster,
    None,
}

impl Framework {
    /// Returns a framework-specific struct that implements the trait `ShuttleInit`
    /// for writing framework-specific dependencies to `Cargo.toml` and generating
    /// boilerplate code in `src/main.rs`.
    pub fn init_config(&self) -> Box<dyn ShuttleInit> {
        match self {
            Framework::ActixWeb => Box::new(ShuttleInitActixWeb),
            Framework::Axum => Box::new(ShuttleInitAxum),
            Framework::Rocket => Box::new(ShuttleInitRocket),
            Framework::Tide => Box::new(ShuttleInitTide),
            Framework::Tower => Box::new(ShuttleInitTower),
            Framework::Poem => Box::new(ShuttleInitPoem),
            Framework::Salvo => Box::new(ShuttleInitSalvo),
            Framework::Serenity => Box::new(ShuttleInitSerenity),
            Framework::Poise => Box::new(ShuttleInitPoise),
            Framework::Warp => Box::new(ShuttleInitWarp),
            Framework::Thruster => Box::new(ShuttleInitThruster),
            Framework::None => Box::new(ShuttleInitNoOp),
        }
    }
}

pub trait ShuttleInit {
    fn get_base_dependencies(&self) -> Vec<&str>;
    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>>;
    fn get_boilerplate_code_for_framework(&self) -> &'static str;
}

pub struct ShuttleInitActixWeb;

impl ShuttleInit for ShuttleInitActixWeb {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["actix-web"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-actix-web",
            HashMap::from([("features", Value::from(Array::from_iter(["web-actix-web"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use actix_web::{get, web::ServiceConfig};
        use shuttle_actix_web::ShuttleActixWeb;
        #[get("/hello")]
        async fn hello_world() -> &'static str {
            "Hello World!"
        }
        #[shuttle_runtime::main]
        async fn actix_web(
        ) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Send + Clone + 'static> {
            let config = move |cfg: &mut ServiceConfig| {
                cfg.service(hello_world);
            };
            Ok(config.into())
        }"#}
    }
}

pub struct ShuttleInitAxum;

impl ShuttleInit for ShuttleInitAxum {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["axum", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-axum",
            HashMap::from([("features", Value::from(Array::from_iter(["web-axum"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use axum::{routing::get, Router};
        async fn hello_world() -> &'static str {
            "Hello, world!"
        }
        #[shuttle_runtime::main]
        async fn axum() -> shuttle_axum::ShuttleAxum {
            let router = Router::new().route("/hello", get(hello_world));
            Ok(router.into())
        }"#}
    }
}

pub struct ShuttleInitRocket;

impl ShuttleInit for ShuttleInitRocket {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["rocket", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-rocket",
            HashMap::from([("features", Value::from(Array::from_iter(["web-rocket"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        #[macro_use]
        extern crate rocket;
        #[get("/")]
        fn index() -> &'static str {
            "Hello, world!"
        }
        #[shuttle_runtime::main]
        async fn rocket() -> shuttle_rocket::ShuttleRocket {
            let rocket = rocket::build().mount("/hello", routes![index]);
            Ok(rocket.into())
        }"#}
    }
}

pub struct ShuttleInitTide;

impl ShuttleInit for ShuttleInitTide {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["tide", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-tide",
            HashMap::from([("features", Value::from(Array::from_iter(["web-tide"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        #[shuttle_runtime::main]
        async fn tide() -> shuttle_tide::ShuttleTide<()> {
            let mut app = tide::new();
            app.with(tide::log::LogMiddleware::new());
            app.at("/hello").get(|_| async { Ok("Hello, world!") });
            Ok(app.into())
        }"#}
    }
}

pub struct ShuttleInitPoem;

impl ShuttleInit for ShuttleInitPoem {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["poem", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-poem",
            HashMap::from([("features", Value::from(Array::from_iter(["web-poem"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use poem::{get, handler, Route};
        use shuttle_poem::ShuttlePoem;
        #[handler]
        fn hello_world() -> &'static str {
            "Hello, world!"
        }
        #[shuttle_runtime::main]
        async fn poem() -> ShuttlePoem<impl poem::Endpoint> {
            let app = Route::new().at("/hello", get(hello_world));
            Ok(app.into())
        }"#}
    }
}

pub struct ShuttleInitSalvo;

impl ShuttleInit for ShuttleInitSalvo {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["salvo", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-salvo",
            HashMap::from([("features", Value::from(Array::from_iter(["web-salvo"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use salvo::prelude::*;
        #[handler]
        async fn hello_world(res: &mut Response) {
            res.render(Text::Plain("Hello, world!"));
        }
        #[shuttle_runtime::main]
        async fn salvo() -> shuttle_salvo::ShuttleSalvo {
            let router = Router::with_path("hello").get(hello_world);
            Ok(router.into())
        }"#}
    }
}

pub struct ShuttleInitSerenity;

impl ShuttleInit for ShuttleInitSerenity {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["anyhow", "shuttle-secrets", "tracing", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([
            (
                "shuttle-serenity",
                HashMap::from([("features", Value::from(Array::from_iter(["bot-serenity"])))]),
            ),
            (
                "serenity",
                HashMap::from([
                    (
                        "features",
                        Value::from(Array::from_iter([
                            "client",
                            "gateway",
                            "rustls_backend",
                            "model",
                        ])),
                    ),
                    ("default-features", Value::from(false)),
                ]),
            ),
        ])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use anyhow::anyhow;
        use serenity::async_trait;
        use serenity::model::channel::Message;
        use serenity::model::gateway::Ready;
        use serenity::prelude::*;
        use shuttle_secrets::SecretStore;
        use tracing::{error, info};
        struct Bot;
        #[async_trait]
        impl EventHandler for Bot {
            async fn message(&self, ctx: Context, msg: Message) {
                if msg.content == "!hello" {
                    if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
                        error!("Error sending message: {:?}", e);
                    }
                }
            }
            async fn ready(&self, _: Context, ready: Ready) {
                info!("{} is connected!", ready.user.name);
            }
        }
        #[shuttle_runtime::main]
        async fn serenity(
            #[shuttle_secrets::Secrets] secret_store: SecretStore,
        ) -> shuttle_serenity::ShuttleSerenity {
            // Get the discord token set in `Secrets.toml`
            let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
                token
            } else {
                return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
            };
            // Set gateway intents, which decides what events the bot will be notified about
            let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
            let client = Client::builder(&token, intents)
                .event_handler(Bot)
                .await
                .expect("Err creating client");
            Ok(client.into())
        }"#}
    }
}

pub struct ShuttleInitPoise;

impl ShuttleInit for ShuttleInitPoise {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["anyhow", "poise", "shuttle-secrets", "tracing", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-poise",
            HashMap::from([("features", Value::from(Array::from_iter(["bot-poise"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use anyhow::Context as _;
        use poise::serenity_prelude as serenity;
        use shuttle_secrets::SecretStore;
        use shuttle_poise::ShuttlePoise;
        struct Data {} // User data, which is stored and accessible in all command invocations
        type Error = Box<dyn std::error::Error + Send + Sync>;
        type Context<'a> = poise::Context<'a, Data, Error>;
        /// Responds with "world!"
        #[poise::command(slash_command)]
        async fn hello(ctx: Context<'_>) -> Result<(), Error> {
            ctx.say("world!").await?;
            Ok(())
        }
        #[shuttle_runtime::main]
        async fn poise(#[shuttle_secrets::Secrets] secret_store: SecretStore) -> ShuttlePoise<Data, Error> {
            // Get the discord token set in `Secrets.toml`
            let discord_token = secret_store
                .get("DISCORD_TOKEN")
                .context("'DISCORD_TOKEN' was not found")?;
            let framework = poise::Framework::builder()
                .options(poise::FrameworkOptions {
                    commands: vec![hello()],
                    ..Default::default()
                })
                .token(discord_token)
                .intents(serenity::GatewayIntents::non_privileged())
                .setup(|ctx, _ready, framework| {
                    Box::pin(async move {
                        poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                        Ok(Data {})
                    })
                })
                .build()
                .await
                .map_err(shuttle_runtime::CustomError::new)?;
            Ok(framework.into())
        }"#}
    }
}

pub struct ShuttleInitTower;

impl ShuttleInit for ShuttleInitTower {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([
            (
                "shuttle-tower",
                HashMap::from([("features", Value::from(Array::from_iter(["web-tower"])))]),
            ),
            (
                "tower",
                HashMap::from([("features", Value::from(Array::from_iter(["full"])))]),
            ),
            (
                "hyper",
                HashMap::from([("features", Value::from(Array::from_iter(["full"])))]),
            ),
        ])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use std::convert::Infallible;
        use std::future::Future;
        use std::pin::Pin;
        use std::task::{Context, Poll};
        #[derive(Clone)]
        struct HelloWorld;
        impl tower::Service<hyper::Request<hyper::Body>> for HelloWorld {
            type Response = hyper::Response<hyper::Body>;
            type Error = Infallible;
            type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;
            fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }
            fn call(&mut self, _req: hyper::Request<hyper::Body>) -> Self::Future {
                let body = hyper::Body::from("Hello, world!");
                let resp = hyper::Response::builder()
                    .status(200)
                    .body(body)
                    .expect("Unable to create the `hyper::Response` object");
                let fut = async { Ok(resp) };
                Box::pin(fut)
            }
        }
        #[shuttle_runtime::main]
        async fn tower() -> shuttle_tower::ShuttleTower<HelloWorld> {
            let service = HelloWorld;
            Ok(service.into())
        }"#}
    }
}

pub struct ShuttleInitWarp;

impl ShuttleInit for ShuttleInitWarp {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["shuttle-warp", "tokio", "warp"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::new()
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use warp::Filter;
        use warp::Reply;
        
        #[shuttle_runtime::main]
        async fn warp() -> shuttle_warp::ShuttleWarp<(impl Reply,)> {
            let route = warp::any().map(|| "Hello, World!");
            Ok(route.boxed().into())
        }"#}
    }
}

pub struct ShuttleInitThruster;

impl ShuttleInit for ShuttleInitThruster {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec!["shuttle-thruster", "tokio"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "thruster",
            HashMap::from([("features", Value::from(Array::from_iter(["hyper_server"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use thruster::{
            context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx, HyperRequest},
            m, middleware_fn, App, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
        };
        
        #[middleware_fn]
        async fn hello(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
            context.body("Hello, World!");
            Ok(context)
        }
        
        #[shuttle_runtime::main]
        async fn thruster() -> shuttle_thruster::ShuttleThruster<HyperServer<Ctx, ()>> {
            let server = HyperServer::new(
                App::<HyperRequest, Ctx, ()>::create(generate_context, ()).get("/hello", m![hello]),
            );
            
            Ok(server.into())
        }"#}
    }
}

pub struct ShuttleInitNoOp;
impl ShuttleInit for ShuttleInitNoOp {
    fn get_base_dependencies(&self) -> Vec<&str> {
        vec![]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        ""
    }
}

/// Interoprates with `cargo` crate and calls `cargo init --libs [path]`.
pub fn cargo_init(path: PathBuf) -> Result<()> {
    let opts = NewOptions::new(None, false, true, path, None, None, None)?;
    let cargo_config = cargo::util::config::Config::default()?;
    let init_result = cargo::ops::init(&opts, &cargo_config)?;

    // Mimic `cargo init` behavior and log status or error to shell
    cargo_config
        .shell()
        .status("Created", format!("{} (shuttle) package", init_result))?;

    Ok(())
}

pub fn build_cargo_file(cargo_contents: String, framework: Framework) -> Result<String> {
    let mut cargo_doc = cargo_contents.parse::<Document>()?;

    cargo_doc["package"]["publish"] = value(false);

    let init_config = framework.init_config();
    let mut cargo_builder = CargoBuilder::new();

    let dependencies = init_config.get_base_dependencies();

    for &dep in dependencies.iter() {
        cargo_builder.add_dependency(Dependency::new(dep.to_owned(), None));
    }

    let dependency_attributes = init_config.get_dependency_attributes();

    for (dependency, attribute) in dependency_attributes {
        for (name, value) in attribute {
            cargo_builder.add_dependency_var(
                Dependency::new(dependency.to_owned(), None),
                name.to_owned(),
                value,
            );
        }
    }

    Ok(cargo_builder.combine(cargo_doc)?.to_string())
}

/// Performs shuttle init on the existing files generated by `cargo init --libs [path]`.
pub fn cargo_shuttle_init(path: PathBuf, framework: Framework) -> Result<()> {
    let cargo_toml_path = path.join("Cargo.toml");
    let cargo_doc = read_to_string(cargo_toml_path.clone())?;
    let cargo_contents = build_cargo_file(cargo_doc, framework);
    let mut cargo_toml = File::create(cargo_toml_path).expect("this one");
    cargo_toml.write_all(cargo_contents?.as_bytes())?;

    // Write boilerplate to `src/lib.rs` file
    let init_config = framework.init_config();
    let lib_path = path.join("src").join("lib.rs");
    let boilerplate = init_config.get_boilerplate_code_for_framework();
    if !boilerplate.is_empty() {
        write_lib_file(boilerplate, &lib_path)?;
    }

    Ok(())
}

/// Writes `boilerplate` code to the specified `lib.rs` file path.
pub fn write_lib_file(boilerplate: &'static str, lib_path: &Path) -> Result<()> {
    let mut lib_file = File::create(lib_path)?;
    lib_file.write_all(boilerplate.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod shuttle_init_tests {
    use super::*;
    use crate::cargo_builder::CargoSection;

    fn get_framework_cargo_init(framework: Framework) -> Document {
        let init_config = framework.init_config();
        let dep_version = Some("1.0".to_owned());
        let dep_feature_version = Some("2.0".to_owned());
        let mut cargo_builder = CargoBuilder::new();

        let dependencies = init_config.get_base_dependencies();

        for &dep in dependencies.iter() {
            cargo_builder.add_dependency(Dependency::new(dep.to_owned(), dep_version.to_owned()));
        }

        let dependency_attributes = init_config.get_dependency_attributes();
        for (dependency, attribute) in dependency_attributes {
            for (name, value) in attribute {
                cargo_builder.add_var(
                    CargoSection::Dependency(Dependency::new(
                        dependency.to_owned(),
                        dep_feature_version.to_owned(),
                    )),
                    name.to_owned(),
                    value,
                );
            }
        }

        let mut base_doc = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"

            [dependencies]
        "#}
        .parse::<Document>()
        .unwrap();

        base_doc["package"]["publish"] = value(false);

        cargo_builder.combine(base_doc).unwrap()
    }

    #[test]
    fn init_build_cargo_dependencies_actix() {
        let framework = Framework::ActixWeb;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            actix-web = "1.0"
            shuttle-actix-web = "1.0"
            tokio = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_axum() {
        let framework = Framework::Axum;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            axum = "1.0"
            shuttle-axum = "1.0"
            tokio = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_rocket() {
        let framework = Framework::Rocket;
        let expected = indoc! {r#"
            [package]
            name = "mproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            rocket = "1.0"
            shuttle-rocket = "1.0"
            tokio = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_tide() {
        let framework = Framework::Tide;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            shuttle-tide = "1.0"
            tokio = "1.0"
            tide = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_tower() {
        let framework = Framework::Tower;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            hyper = { version = "2.0", features = ["full"] }
            shuttle-tower = "1.0"
            tokio = "1.0"
            tower = { version = "2.0", features = ["full"] }
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_poem() {
        let framework = Framework::Poem;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            poem = "1.0"
            shuttle-poem = "1.0"
            tokio = "1.0"
            [package]
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_salvo() {
        let framework = Framework::Salvo;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            salvo = "1.0"
            shuttle-salvo = "1.0"
            tokio = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_serenity() {
        let framework = Framework::Serenity;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            anyhow = "1.0"
            serenity = { version = "2.0", default-features = false, features = ["client", "gateway", "rustls_backend", "model"] }
            shuttle-secrets = "1.0"
            shuttle-serenity = "1.0"
            tokio = "1.0"
            tracing = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_poise() {
        let framework = Framework::Poise;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            anyhow = "1.0"
            poise = "1.0"
            shuttle-poise = "1.0"
            shuttle-secrets = "1.0"
            tokio = "1.0"
            tracing = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

    #[test]
    fn init_build_cargo_dependencies_thruster() {
        let framework = Framework::Thruster;
        let expected = indoc! {r#"
            [package]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            shuttle-runtime = "1.0"
            shuttle-thruster = "1.0"
            thruster = { version = "2.0", features = ["hyper_server"] }
            tokio = "1.0"
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }
}
