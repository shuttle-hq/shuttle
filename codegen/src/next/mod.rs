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

pub(crate) struct App {
    endpoints: Vec<Endpoint>,
}

impl ToTokens for App {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self { endpoints } = self;

        let app = quote!(
            async fn __app<B>(request: http::Request<B>) -> axum::response::Response
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

pub(crate) fn wasi_bindings(app: App) -> proc_macro2::TokenStream {
    quote!(
        #app

        #[no_mangle]
        #[allow(non_snake_case)]
        pub extern "C" fn __SHUTTLE_Axum_call(
            fd_3: std::os::wasi::prelude::RawFd,
            fd_4: std::os::wasi::prelude::RawFd,
        ) {
            use axum::body::HttpBody;
            use std::io::{Read, Write};
            use std::os::wasi::io::FromRawFd;

            println!("inner handler awoken; interacting with fd={fd_3},{fd_4}");

            // file descriptor 3 for reading and writing http parts
            let mut parts_fd = unsafe { std::fs::File::from_raw_fd(fd_3) };

            let reader = std::io::BufReader::new(&mut parts_fd);

            // deserialize request parts from rust messagepack
            let wrapper: shuttle_common::wasm::RequestWrapper = rmp_serde::from_read(reader).unwrap();

            // file descriptor 4 for reading and writing http body
            let mut body_fd = unsafe { std::fs::File::from_raw_fd(fd_4) };

            // read body from host
            let mut body_buf = Vec::new();
            let mut c_buf: [u8; 1] = [0; 1];
            loop {
                body_fd.read(&mut c_buf).unwrap();
                if c_buf[0] == 0 {
                    break;
                } else {
                    body_buf.push(c_buf[0]);
                }
            }

            let request: http::Request<axum::body::Body> = wrapper
                .into_request_builder()
                .body(body_buf.into())
                .unwrap();

            println!("inner router received request: {:?}", &request);
            let res = futures_executor::block_on(__app(request));

            let (parts, mut body) = res.into_parts();

            // wrap and serialize response parts as rmp
            let response_parts = shuttle_common::wasm::ResponseWrapper::from(parts).into_rmp();

            // write response parts
            parts_fd.write_all(&response_parts).unwrap();

            // write body if there is one
            if let Some(body) = futures_executor::block_on(body.data()) {
                body_fd.write_all(body.unwrap().as_ref()).unwrap();
            }
            // signal to the reader that end of file has been reached
            body_fd.write(&[0]).unwrap();
        }
    )
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
            async fn __app<B>(request: http::Request<B>) -> axum::response::Response
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
