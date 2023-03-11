use std::collections::HashMap;
use std::fs::{read_to_string, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::cargo_builder::{CargoBuilder, Dependency};
use anyhow::Result;
use cargo::ops::NewOptions;
use cargo_edit::{find, get_latest_dependency, registry_url};
use indoc::indoc;
use toml_edit::{value, Array, Document, Item, Table, Value};
use url::{PathSegmentsMut, Url};

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
    /// boilerplate code in `src/lib.rs`.
    pub fn init_config(&self) -> Box<dyn ShuttleInit> {
        match self {
            Framework::ActixWeb => Box::new(ShuttleInitActixWeb),
            //Framework::Axum => Box::new(ShuttleInitAxum),
            //Framework::Rocket => Box::new(ShuttleInitRocket),
            //Framework::Tide => Box::new(ShuttleInitTide),
            //Framework::Tower => Box::new(ShuttleInitTower),
            //Framework::Poem => Box::new(ShuttleInitPoem),
            //Framework::Salvo => Box::new(ShuttleInitSalvo),
            //Framework::Serenity => Box::new(ShuttleInitSerenity),
            //Framework::Poise => Box::new(ShuttleInitPoise),
            //Framework::Warp => Box::new(ShuttleInitWarp),
            //Framework::Thruster => Box::new(ShuttleInitThruster),
            Framework::None => Box::new(ShuttleInitNoOp),
            _ => panic!(" tihs should never happen"),
        }
    }
}

pub trait ShuttleInit {
    fn get_minimum_dependencies(&self) -> Vec<&str>;
    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>>; // HashMap<&str, vec![&str]>;
    fn get_boilerplate_code_for_framework(&self) -> &'static str;
}

pub struct ShuttleInitActixWeb;

impl ShuttleInit for ShuttleInitActixWeb {
    fn get_minimum_dependencies(&self) -> Vec<&str> {
        vec!["actix-web"]
    }

    fn get_dependency_attributes(&self) -> HashMap<&str, HashMap<&str, Value>> {
        HashMap::from([(
            "shuttle-service",
            HashMap::from([("features", Value::from(Array::from_iter(["web-actix-web"])))]),
        )])
    }

    fn get_boilerplate_code_for_framework(&self) -> &'static str {
        indoc! {r#"
        use actix_web::{get, web::ServiceConfig};
        use shuttle_service::ShuttleActixWeb;

        #[get("/hello")]
        async fn hello_world() -> &'static str {
            "Hello World!"
        }

        #[shuttle_service::main]
        async fn actix_web(
        ) -> ShuttleActixWeb<impl FnOnce(&mut ServiceConfig) + Sync + Send + Clone + 'static> {
            Ok(move |cfg: &mut ServiceConfig| {
                cfg.service(hello_world);
            })
        }"#}
    }
}

//pub struct ShuttleInitAxum;

//impl ShuttleInit for ShuttleInitAxum {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//// Gets current version for the dependency
//set_key_value_dependency_version(
//"axum",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//// `shuttle-service = { features = ["web-axum"] }`
////set_inline_table_dependency_features(
////"shuttle-service",
////dependencies,
////vec!["web-axum".to_string()],
////);

//// sync_wrapper = { version =
////set_key_value_dependency_version(
////"sync_wrapper",
////dependencies,
////manifest_path,
////url,
////false,
////get_dependency_version_fn,
////);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use axum::{routing::get, Router};
//use sync_wrapper::SyncWrapper;

//async fn hello_world() -> &'static str {
//"Hello, world!"
//}

//#[shuttle_service::main]
//async fn axum() -> shuttle_service::ShuttleAxum {
//let router = Router::new().route("/hello", get(hello_world));
//let sync_wrapper = SyncWrapper::new(router);

//Ok(sync_wrapper)
//}"#}
//}
//}

//pub struct ShuttleInitRocket;

//impl ShuttleInit for ShuttleInitRocket {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_key_value_dependency_version(
//"rocket",
//dependencies,
//manifest_path,
//url,
//true,
//get_dependency_version_fn,
//);

//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-rocket".to_string()],
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//#[macro_use]
//extern crate rocket;

//#[get("/")]
//fn index() -> &'static str {
//"Hello, world!"
//}

//#[shuttle_service::main]
//async fn rocket() -> shuttle_service::ShuttleRocket {
//let rocket = rocket::build().mount("/hello", routes![index]);

//Ok(rocket)
//}"#}
//}
//}

//pub struct ShuttleInitTide;

//impl ShuttleInit for ShuttleInitTide {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-tide".to_string()],
//);

//set_key_value_dependency_version(
//"tide",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//#[shuttle_service::main]
//async fn tide() -> shuttle_service::ShuttleTide<()> {
//let mut app = tide::new();
//app.with(tide::log::LogMiddleware::new());

//app.at("/hello").get(|_| async { Ok("Hello, world!") });

//Ok(app)
//}"#}
//}
//}

//pub struct ShuttleInitPoem;

//impl ShuttleInit for ShuttleInitPoem {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-poem".to_string()],
//);

//set_key_value_dependency_version(
//"poem",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use poem::{get, handler, Route};

//#[handler]
//fn hello_world() -> &'static str {
//"Hello, world!"
//}

//#[shuttle_service::main]
//async fn poem() -> shuttle_service::ShuttlePoem<impl poem::Endpoint> {
//let app = Route::new().at("/hello", get(hello_world));

//Ok(app)
//}"#}
//}
//}

//pub struct ShuttleInitSalvo;

//impl ShuttleInit for ShuttleInitSalvo {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-salvo".to_string()],
//);

//set_key_value_dependency_version(
//"salvo",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use salvo::prelude::*;

//#[handler]
//async fn hello_world(res: &mut Response) {
//res.render(Text::Plain("Hello, World!"));
//}

//#[shuttle_service::main]
//async fn salvo() -> shuttle_service::ShuttleSalvo {
//let router = Router::new().get(hello_world);

//Ok(router)
//}"#}
//}
//}

//pub struct ShuttleInitSerenity;

//impl ShuttleInit for ShuttleInitSerenity {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["bot-serenity".to_string()],
//);

//set_key_value_dependency_version(
//"anyhow",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_inline_table_dependency_version(
//"serenity",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//dependencies["serenity"]["default-features"] = value(false);

//set_inline_table_dependency_features(
//"serenity",
//dependencies,
//vec![
//"client".into(),
//"gateway".into(),
//"rustls_backend".into(),
//"model".into(),
//],
//);

//set_key_value_dependency_version(
//"shuttle-secrets",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_key_value_dependency_version(
//"tracing",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use anyhow::anyhow;
//use serenity::async_trait;
//use serenity::model::channel::Message;
//use serenity::model::gateway::Ready;
//use serenity::prelude::*;
//use shuttle_secrets::SecretStore;
//use tracing::{error, info};

//struct Bot;

//#[async_trait]
//impl EventHandler for Bot {
//async fn message(&self, ctx: Context, msg: Message) {
//if msg.content == "!hello" {
//if let Err(e) = msg.channel_id.say(&ctx.http, "world!").await {
//error!("Error sending message: {:?}", e);
//}
//}
//}

//async fn ready(&self, _: Context, ready: Ready) {
//info!("{} is connected!", ready.user.name);
//}
//}

//#[shuttle_service::main]
//async fn serenity(
//#[shuttle_secrets::Secrets] secret_store: SecretStore,
//) -> shuttle_service::ShuttleSerenity {
//// Get the discord token set in `Secrets.toml`
//let token = if let Some(token) = secret_store.get("DISCORD_TOKEN") {
//token
//} else {
//return Err(anyhow!("'DISCORD_TOKEN' was not found").into());
//};

//// Set gateway intents, which decides what events the bot will be notified about
//let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

//let client = Client::builder(&token, intents)
//.event_handler(Bot)
//.await
//.expect("Err creating client");

//Ok(client)
//}"#}
//}
//}

//pub struct ShuttleInitPoise;

//impl ShuttleInit for ShuttleInitPoise {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["bot-poise".to_string()],
//);

//set_key_value_dependency_version(
//"anyhow",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_key_value_dependency_version(
//"poise",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_key_value_dependency_version(
//"shuttle-secrets",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_key_value_dependency_version(
//"tracing",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use anyhow::Context as _;
//use poise::serenity_prelude as serenity;
//use shuttle_secrets::SecretStore;
//use shuttle_service::ShuttlePoise;

//struct Data {} // User data, which is stored and accessible in all command invocations
//type Error = Box<dyn std::error::Error + Send + Sync>;
//type Context<'a> = poise::Context<'a, Data, Error>;

///// Responds with "world!"
//#[poise::command(slash_command)]
//async fn hello(ctx: Context<'_>) -> Result<(), Error> {
//ctx.say("world!").await?;
//Ok(())
//}

//#[shuttle_service::main]
//async fn poise(#[shuttle_secrets::Secrets] secret_store: SecretStore) -> ShuttlePoise<Data, Error> {
//// Get the discord token set in `Secrets.toml`
//let discord_token = secret_store
//.get("DISCORD_TOKEN")
//.context("'DISCORD_TOKEN' was not found")?;

//let framework = poise::Framework::builder()
//.options(poise::FrameworkOptions {
//commands: vec![hello()],
//..Default::default()
//})
//.token(discord_token)
//.intents(serenity::GatewayIntents::non_privileged())
//.setup(|ctx, _ready, framework| {
//Box::pin(async move {
//poise::builtins::register_globally(ctx, &framework.options().commands).await?;
//Ok(Data {})
//})
//})
//.build()
//.await
//.map_err(shuttle_service::error::CustomError::new)?;

//Ok(framework)
//}"#}
//}
//}

//pub struct ShuttleInitTower;

//impl ShuttleInit for ShuttleInitTower {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-tower".to_string()],
//);

//set_inline_table_dependency_version(
//"tower",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_inline_table_dependency_features("tower", dependencies, vec!["full".to_string()]);

//set_inline_table_dependency_version(
//"hyper",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_inline_table_dependency_features("hyper", dependencies, vec!["full".to_string()]);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use std::convert::Infallible;
//use std::future::Future;
//use std::pin::Pin;
//use std::task::{Context, Poll};

//#[derive(Clone)]
//struct HelloWorld;

//impl tower::Service<hyper::Request<hyper::Body>> for HelloWorld {
//type Response = hyper::Response<hyper::Body>;
//type Error = Infallible;
//type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + Sync>>;

//fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
//Poll::Ready(Ok(()))
//}

//fn call(&mut self, _req: hyper::Request<hyper::Body>) -> Self::Future {
//let body = hyper::Body::from("Hello, world!");
//let resp = hyper::Response::builder()
//.status(200)
//.body(body)
//.expect("Unable to create the `hyper::Response` object");

//let fut = async { Ok(resp) };

//Box::pin(fut)
//}
//}

//#[shuttle_service::main]
//async fn tower() -> Result<HelloWorld, shuttle_service::Error> {
//Ok(HelloWorld)
//}"#}
//}
//}

//pub struct ShuttleInitWarp;

//impl ShuttleInit for ShuttleInitWarp {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-warp".to_string()],
//);

//set_key_value_dependency_version(
//"warp",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use warp::Filter;
//use warp::Reply;

//#[shuttle_service::main]
//async fn warp() -> shuttle_service::ShuttleWarp<(impl Reply,)> {
//let route = warp::any().map(|| "Hello, World");
//Ok(route.boxed())
//}"#}
//}
//}

//pub struct ShuttleInitThruster;

//impl ShuttleInit for ShuttleInitThruster {
//fn set_cargo_dependencies(
//&self,
//dependencies: &mut Table,
//manifest_path: &Path,
//url: &Url,
//get_dependency_version_fn: GetDependencyVersionFn,
//) {
//set_inline_table_dependency_features(
//"shuttle-service",
//dependencies,
//vec!["web-thruster".to_string()],
//);

//set_inline_table_dependency_version(
//"thruster",
//dependencies,
//manifest_path,
//url,
//false,
//get_dependency_version_fn,
//);

//set_inline_table_dependency_features(
//"thruster",
//dependencies,
//vec!["hyper_server".to_string()],
//);
//}

//fn get_boilerplate_code_for_framework(&self) -> &'static str {
//indoc! {r#"
//use thruster::{
//context::basic_hyper_context::{generate_context, BasicHyperContext as Ctx, HyperRequest},
//m, middleware_fn, App, HyperServer, MiddlewareNext, MiddlewareResult, ThrusterServer,
//};

//#[middleware_fn]
//async fn hello(mut context: Ctx, _next: MiddlewareNext<Ctx>) -> MiddlewareResult<Ctx> {
//context.body("Hello, World!");
//Ok(context)
//}

//#[shuttle_service::main]
//async fn thruster() -> shuttle_service::ShuttleThruster<HyperServer<Ctx, ()>> {
//Ok(HyperServer::new(
//App::<HyperRequest, Ctx, ()>::create(generate_context, ()).get("/hello", m![hello]),
//))
//}
//"#}
//}
//}

pub struct ShuttleInitNoOp;
impl ShuttleInit for ShuttleInitNoOp {
    fn get_minimum_dependencies(&self) -> Vec<&str> {
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
    cargo_doc["lib"] = Item::Table(Table::new());

    let init_config = framework.init_config();
    let mut cargo_builder = CargoBuilder::new();

    let dependencies = init_config.get_minimum_dependencies();

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
    let mut cargo_doc = read_to_string(path.clone())?; //.parse::<Document>()?;
    let cargo_contents = build_cargo_file(cargo_doc, framework);
    // save the file here

    panic!("end");

    Ok(())
}

#[cfg(test)]
mod shuttle_init_tests {
    use super::*;
    use crate::cargo_builder::CargoSection;

    fn get_mock_dependency(name: &str, version: Option<String>) -> Dependency {
        Dependency::new(name.to_owned(), None)   
    }

    fn get_framework_cargo_init(framework: Framework) -> Document {
        let init_config = framework.init_config();
        let dep_version = Some("1.0".to_owned());
        let dep_feature_version = Some("2.0".to_owned());
        let mut cargo_builder = CargoBuilder::new();

        let dependencies = init_config.get_minimum_dependencies();

        for &dep in dependencies.iter() {
            cargo_builder.add_dependency(Dependency::new(dep.to_owned(), dep_version.to_owned()));
        }

        let dependency_attributes = init_config.get_dependency_attributes();
        for (dependency, attribute) in dependency_attributes {
            for (name, value) in attribute {
                cargo_builder.add_var(
                    CargoSection::Dependency(Dependency::new(dependency.to_owned(), dep_feature_version.to_owned())),
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

        let doc = cargo_builder.combine(base_doc).unwrap();
        doc

    }

    #[test]
    fn test_set_cargo_dependencies_actix() {

        let framework = Framework::ActixWeb;
        let expected = indoc! {r#"
            [ppackage]
            name = "myproject"
            version = "0.1.0"
            edition = "2021"
            publish = false

            [dependencies]
            actix-web = "1.0"
            shuttle-service = { features = ["web-actix-web"], version = "2.0" }
        "#};

        let doc = get_framework_cargo_init(framework);

        assert_eq!(doc.to_string(), expected);
    }

}
