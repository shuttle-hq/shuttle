use http::Method;
use quote::{quote, ToTokens};
use syn::{Ident, LitStr};

struct Endpoint {
    route: LitStr,
    method: Method,
    function: Ident,
}

impl ToTokens for Endpoint {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            route,
            method,
            function,
        } = self;

        let method = match *method {
            Method::GET => quote!(get),
            Method::POST => quote!(post),
            Method::DELETE => quote!(delete),
            Method::PUT => quote!(put),
            Method::OPTIONS => quote!(options),
            Method::CONNECT => quote!(connect),
            Method::HEAD => quote!(head),
            Method::TRACE => quote!(trace),
            Method::PATCH => quote!(patch),
            _ => quote!(extension),
        };

        let route = quote!(.route(#route, axum::routing::#method(#function)));

        route.to_tokens(tokens);
    }
}

struct App {
    endpoints: Vec<Endpoint>,
}

impl ToTokens for App {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { endpoints } = self;

        let app = quote!(
            async fn app<B>(request: http::Request<B>) -> axum::response::Response
            where
                B: axum::body::HttpBody + Send + 'static,
            {
                use tower_service::Service;

                let mut router = axum::Router::new()
                    #(#endpoints)*
                    .into_service();

                let response = router.call(request).await.unwrap();

                response
            }
        );

        app.to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use http::Method;
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::parse_quote;

    use crate::next::App;

    use super::Endpoint;

    #[test]
    fn endpoint_to_token() {
        let endpoint = Endpoint {
            route: parse_quote!("/hello"),
            method: Method::GET,
            function: parse_quote!(hello),
        };

        let actual = quote!(#endpoint);
        let expected = quote!(.route("/hello", axum::routing::get(hello)));

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn app_to_token() {
        let app = App {
            endpoints: vec![
                Endpoint {
                    route: parse_quote!("/hello"),
                    method: Method::GET,
                    function: parse_quote!(hello),
                },
                Endpoint {
                    route: parse_quote!("/goodbye"),
                    method: Method::POST,
                    function: parse_quote!(goodbye),
                },
            ],
        };

        let actual = quote!(#app);
        let expected = quote!(
            async fn app<B>(request: http::Request<B>) -> axum::response::Response
            where
                B: axum::body::HttpBody + Send + 'static,
            {
                use tower_service::Service;

                let mut router = axum::Router::new()
                    .route("/hello", axum::routing::get(hello))
                    .route("/goodbye", axum::routing::post(goodbye))
                    .into_service();

                let response = router.call(request).await.unwrap();

                response
            }
        );

        assert_eq!(actual.to_string(), expected.to_string());
    }
}
