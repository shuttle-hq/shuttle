#[cfg(feature = "next")]
mod next;
#[cfg(feature = "frameworks")]
mod shuttle_main;

/// Helper macro that generates the entrypoint required by any service - likely the only macro you need in this crate.
///
/// ## Without shuttle managed resources
/// The simplest usage is when your service does not require any shuttle managed resources, so you only need to return a shuttle supported service:
///
/// ```rust,no_run
/// use shuttle_rocket::ShuttleRocket;
///
/// #[shuttle_rocket::main]
/// async fn rocket() -> ShuttleRocket {
///     let rocket = rocket::build();
///
///     Ok(rocket.into())
/// }
/// ```
///
/// ## Shuttle supported services
/// The following types can be returned from a `#[shuttle_service::main]` function and enjoy first class service support in shuttle.
///
/// | Return type       | Crate                                                          | Service                                                                          | Version    | Example                                                                                 |
/// | ----------------- | -------------------------------------------------------------- | -------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------------------------------------- |
/// | `ShuttleActixWeb` | [shuttle-actix-web](https://crates.io/crates/shuttle-actix-web)| [actix-web](https://docs.rs/actix-web/4.3)                                       | 4.3        | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/actix-web/hello-world)|
/// | `ShuttleAxum`     | [shuttle-axum](https://crates.io/crates/shuttle-axum)          | [axum](https://docs.rs/axum/0.7)                                                 | 0.7        | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/axum/hello-world)     |
/// | `ShuttlePoem`     | [shuttle-poem](https://crates.io/crates/shuttle-poem)          | [poem](https://docs.rs/poem/2.0)                                                 | 2.0        | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/poem/hello-world)     |
/// | `ShuttleRocket`   | [shuttle-rocket](https://crates.io/crates/shuttle-rocket)      | [rocket](https://docs.rs/rocket/0.5)                                             | 0.5        | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/rocket/hello-world)   |
/// | `ShuttleSalvo`    | [shuttle-salvo](https://crates.io/crates/shuttle-salvo)        | [salvo](https://docs.rs/salvo/0.63)                                              | 0.63       | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/salvo/hello-world)    |
/// | `ShuttleSerenity` | [shuttle-serenity](https://crates.io/crates/shuttle-serenity   | [serenity](https://docs.rs/serenity/0.12) and [poise](https://docs.rs/poise/0.6) | 0.12       | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/serenity/hello-world) |
/// | `ShuttleThruster` | [shuttle-thruster](https://crates.io/crates/shuttle-thruster)  | [thruster](https://docs.rs/thruster/1.3)                                         | 1.3        | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/thruster/hello-world) |
/// | `ShuttleTower`    | [shuttle-tower](https://crates.io/crates/shuttle-tower)        | [tower](https://docs.rs/tower/0.4)                                               | 0.4        | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/tower/hello-world)    |
/// | `ShuttleTide`     | [shuttle-tide](https://crates.io/crates/shuttle-tide)          | [tide](https://docs.rs/tide/0.16)                                                | 0.16       | [GitHub](https://github.com/shuttle-hq/shuttle-examples/tree/main/tide/hello-world)     |
///
/// ## Getting shuttle managed resources
/// Shuttle is able to manage resource dependencies for you. These resources are passed in as inputs to your `#[shuttle_runtime::main]` function and are configured using attributes:
/// ```rust,no_run
/// use sqlx::PgPool;
/// use shuttle_rocket::ShuttleRocket;
///
/// struct MyState(PgPool);
///
/// #[shuttle_runtime::main]
/// async fn rocket(#[shuttle_shared_db::Postgres] pool: PgPool) -> ShuttleRocket {
///     let state = MyState(pool);
///     let rocket = rocket::build().manage(state);
///
///     Ok(rocket.into())
/// }
/// ```
///
/// More [shuttle managed resources can be found here](https://github.com/shuttle-hq/shuttle/tree/main/resources)
#[cfg(feature = "frameworks")]
#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn main(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    shuttle_main::r#impl(attr, item)
}

/// Generates a wasm32-wasi module containing an Axum router with your endpoints, which is passed as a
/// hyper::service::Service to a hyper::Server.
///
/// ## Example
///
/// ```
/// shuttle_next::app! {
///     use futures::TryStreamExt;
///     use tracing::debug;
///     use shuttle_next::body::StreamBody;
///     use shuttle_next::extract::BodyStream;
///     use shuttle_next::response::{Response, IntoResponse};
///
///     #[shuttle_next::endpoint(method = get, route = "/")]
///     async fn hello() -> &'static str {
///         "Hello, World!"
///     }
///
///     // We can also use tracing/log macros directly:
///     #[shuttle_next::endpoint(method = get, route = "/goodbye")]
///     async fn goodbye() -> &'static str {
///         debug!("goodbye endpoint called");
///         "Goodbye, World!"
///     }
///
///     // We can also extract the http body in our handlers.
///     // The endpoint below takes the body from the request using the axum `BodyStream`
///     // extractor, lazily maps its bytes to uppercase and streams it back in our response:
///     #[shuttle_next::endpoint(method = post, route = "/uppercase")]
///     async fn uppercase(body: BodyStream) -> impl IntoResponse {
///         let chunk_stream = body.map_ok(|chunk| {
///             chunk
///                 .iter()
///                 .map(|byte| byte.to_ascii_uppercase())
///                 .collect::<Vec<u8>>()
///         });
///         Response::new(StreamBody::new(chunk_stream))
///     }
/// }
/// ```
#[cfg(feature = "next")]
#[proc_macro_error::proc_macro_error]
#[proc_macro]
pub fn app(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    use next::App;
    use syn::{parse_macro_input, File};

    let mut file = parse_macro_input!(item as File);

    let app = App::from_file(&mut file);
    let bindings = next::wasi_bindings(app);

    quote::quote!(
        #file
        #bindings
    )
    .into()
}
