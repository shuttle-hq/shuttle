#[cfg(feature = "next")]
mod next;
#[cfg(feature = "frameworks")]
mod shuttle_main;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;

/// Setups up the `shuttle_runtime` to be executed. Allows
/// for user to setup a runtime without needing to explicitly
/// create one.
///
/// # Example
///
/// ```
/// #[shuttle_runtime::main]
/// async fn entry_point() {
///     todo!();
/// }
/// ```
#[cfg(feature = "frameworks")]
#[proc_macro_error]
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    shuttle_main::r#impl(attr, item)
}

/// Setups up a `shuttle_app` to be generated. Allows
/// for user to setup a `next` application.
///
/// # Example
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
#[proc_macro_error]
#[proc_macro]
pub fn app(item: TokenStream) -> TokenStream {
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
