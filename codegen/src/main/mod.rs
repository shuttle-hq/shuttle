use proc_macro::TokenStream;
use proc_macro_error::emit_error;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Attribute, FnArg, Ident, ItemFn, Pat, Path,
    ReturnType, Signature, Stmt, Type,
};

pub(crate) fn r#impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut fn_decl = parse_macro_input!(item as ItemFn);

    let wrapper = Wrapper::from_item_fn(&mut fn_decl);

    let expanded = quote! {
        #wrapper

        fn __binder(
            service: Box<dyn shuttle_service::Service>,
            addr: std::net::SocketAddr,
            runtime: &shuttle_service::Runtime,
        ) -> shuttle_service::ServeHandle {
            runtime.spawn(async move { service.bind(addr).await })
        }

        #fn_decl

        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut shuttle_service::Bootstrapper {
            let builder: shuttle_service::StateBuilder<Box<dyn shuttle_service::Service>> =
                |factory, runtime, logger| Box::pin(__shuttle_wrapper(factory, runtime, logger));

            let bootstrapper = shuttle_service::Bootstrapper::new(
                builder,
                __binder,
                shuttle_service::Runtime::new().unwrap(),
            );

            let boxed = Box::new(bootstrapper);
            Box::into_raw(boxed)
        }
    };

    expanded.into()
}

struct Wrapper {
    fn_ident: Ident,
    fn_inputs: Vec<Input>,
}

#[derive(Debug, PartialEq)]
struct Input {
    /// The identifier for a resource input
    ident: Ident,

    /// The shuttle_service path to the builder for this resource
    builder: Path,
}

impl Wrapper {
    pub(crate) fn from_item_fn(item_fn: &mut ItemFn) -> Self {
        let inputs: Vec<_> = item_fn
            .sig
            .inputs
            .iter_mut()
            .filter_map(|input| match input {
                FnArg::Receiver(_) => None,
                FnArg::Typed(typed) => Some(typed),
            })
            .filter_map(|typed| match typed.pat.as_ref() {
                Pat::Ident(ident) => Some((ident, typed.attrs.drain(..).collect())),
                _ => None,
            })
            .filter_map(|(pat_ident, attrs)| {
                match attribute_to_path(attrs) {
                    Ok(builder) => Some(Input {
                        ident: pat_ident.ident.clone(),
                        builder,
                    }),
                    Err(err) => {
                        emit_error!(pat_ident, err; hint = pat_ident.span() => "Try adding a config like `#[shared::Postgres]`");
                        None
                    }
                }
            })
            .collect();

        check_return_type(&item_fn.sig);

        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_inputs: inputs,
        }
    }
}

fn check_return_type(signature: &Signature) {
    match &signature.output {
        ReturnType::Default => emit_error!(
            signature,
            "shuttle_service::main functions need to return a service";
            hint = "See the docs for services with first class support";
            doc = "https://docs.rs/shuttle-service/latest/shuttle_service/attr.main.html#shuttle-supported-services"
        ),
        ReturnType::Type(_, r#type) => match r#type.as_ref() {
            Type::Path(_) => {}
            _ => emit_error!(
                r#type,
                "shuttle_service::main functions need to return a first class service or 'Result<impl Service, shuttle_service::Error>";
                hint = "See the docs for services with first class support";
                doc = "https://docs.rs/shuttle-service/latest/shuttle_service/attr.main.html#shuttle-supported-services"
            ),
        },
    }
}

fn attribute_to_path(attrs: Vec<Attribute>) -> Result<Path, String> {
    if attrs.is_empty() {
        return Err("resource needs an attribute configuration".to_string());
    }

    let builder = attrs[0].path.clone();

    Ok(builder)
}

impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let fn_ident = &self.fn_ident;
        let fn_inputs: Vec<_> = self.fn_inputs.iter().map(|i| i.ident.clone()).collect();
        let fn_inputs_builder: Vec<_> = self.fn_inputs.iter().map(|i| i.builder.clone()).collect();

        let factory_ident: Ident = if self.fn_inputs.is_empty() {
            parse_quote!(_factory)
        } else {
            parse_quote!(factory)
        };

        let extra_imports: Option<Stmt> = if self.fn_inputs.is_empty() {
            None
        } else {
            Some(parse_quote!(
                use shuttle_service::ResourceBuilder;
            ))
        };

        let wrapper = quote! {
            async fn __shuttle_wrapper(
                #factory_ident: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: shuttle_service::logger::Logger,
            ) -> Result<Box<dyn shuttle_service::Service>, shuttle_service::Error> {
                use shuttle_service::tracing_subscriber::prelude::*;
                #extra_imports

                runtime.spawn_blocking(move || {
                    let fmt_layer = shuttle_service::tracing_subscriber::fmt::layer();
                    let filter_layer =
                        shuttle_service::tracing_subscriber::EnvFilter::try_from_default_env()
                            .or_else(|_| shuttle_service::tracing_subscriber::EnvFilter::try_new("INFO"))
                            .unwrap();

                    shuttle_service::tracing_subscriber::registry()
                        .with(filter_layer)
                        .with(logger)
                        .with(fmt_layer) // TODO: eventually remove 'fmt_layer` once `log_layer` is done
                        .init(); // .init() sets the subscriber as the global default and also adds a compatibility layer for `log::Record`s

                })
                .await
                .map_err(|e| {
                    if e.is_panic() {
                        let mes = e
                            .into_panic()
                            .downcast_ref::<&str>()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "<no panic message>".to_string());

                        shuttle_service::Error::BuildPanic(mes)
                    } else {
                        shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
                    }
                })?;


                #(let #fn_inputs = shuttle_service::#fn_inputs_builder::new().build(#factory_ident, runtime).await?;)*

                runtime.spawn(async {
                    #fn_ident(#(#fn_inputs),*)
                        .await
                        .map(|ok| Box::new(ok) as Box<dyn shuttle_service::Service>)
                })
                    .await
                    .map_err(|e| {
                        if e.is_panic() {
                            let mes = e
                                .into_panic()
                                .downcast_ref::<&str>()
                                .map(|x| x.to_string())
                                .unwrap_or_else(|| "<no panic message>".to_string());

                            shuttle_service::Error::BuildPanic(mes)
                        } else {
                            shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
                        }
                    })?
            }
        };

        wrapper.to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::{parse_quote, Ident};

    use super::{Input, Wrapper};

    #[test]
    fn from_with_return() {
        let mut input = parse_quote!(
            async fn complex() -> ShuttleAxum {}
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(complex);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_inputs, Vec::<Input>::new());
    }

    #[test]
    fn output_with_return() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_inputs: Vec::new(),
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                _factory: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: Box<dyn shuttle_service::log::Log>,
            ) -> Result<Box<dyn shuttle_service::Service>, shuttle_service::Error> {
                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(logger)
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                })
                .await
                .map_err(|e| {
                    if e.is_panic() {
                        let mes = e
                            .into_panic()
                            .downcast_ref::<&str>()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "<no panic message>".to_string());

                        shuttle_service::Error::BuildPanic(mes)
                    } else {
                        shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
                    }
                })?;

                runtime.spawn(async {
                    complex()
                        .await
                        .map(|ok| Box::new(ok) as Box<dyn shuttle_service::Service>)
                })
                .await
                .map_err(|e| {
                    if e.is_panic() {
                        let mes = e
                            .into_panic()
                            .downcast_ref::<&str>()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "<no panic message>".to_string());

                        shuttle_service::Error::BuildPanic(mes)
                    } else {
                        shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
                    }
                })?
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_inputs() {
        let mut input = parse_quote!(
            async fn complex(#[shared::Postgres] pool: PgPool) -> ShuttleTide {}
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(complex);
        let expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: parse_quote!(shared::Postgres),
        }];

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_inputs, expected_inputs);

        // Make sure attributes was removed from input
        if let syn::FnArg::Typed(param) = input.sig.inputs.first().unwrap() {
            assert!(
                param.attrs.is_empty(),
                "some attributes were not removed: {:?}",
                param.attrs
            );
        } else {
            panic!("expected first input to be typed")
        }
    }

    #[test]
    fn output_with_inputs() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_inputs: vec![
                Input {
                    ident: parse_quote!(pool),
                    builder: parse_quote!(shared::Postgres),
                },
                Input {
                    ident: parse_quote!(redis),
                    builder: parse_quote!(shared::Redis),
                },
            ],
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                factory: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: Box<dyn shuttle_service::log::Log>,
            ) -> Result<Box<dyn shuttle_service::Service>, shuttle_service::Error> {
                use shuttle_service::ResourceBuilder;

                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(logger)
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                })
                .await
                .map_err(|e| {
                    if e.is_panic() {
                        let mes = e
                            .into_panic()
                            .downcast_ref::<&str>()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "<no panic message>".to_string());

                        shuttle_service::Error::BuildPanic(mes)
                    } else {
                        shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
                    }
                })?;

                let pool = shuttle_service::shared::Postgres::new().build(factory, runtime).await?;
                let redis = shuttle_service::shared::Redis::new().build(factory, runtime).await?;

                runtime.spawn(async {
                    complex(pool, redis)
                        .await
                        .map(|ok| Box::new(ok) as Box<dyn shuttle_service::Service>)
                })
                .await
                .map_err(|e| {
                    if e.is_panic() {
                        let mes = e
                            .into_panic()
                            .downcast_ref::<&str>()
                            .map(|x| x.to_string())
                            .unwrap_or_else(|| "<no panic message>".to_string());

                        shuttle_service::Error::BuildPanic(mes)
                    } else {
                        shuttle_service::Error::Custom(shuttle_service::error::CustomError::new(e))
                    }
                })?
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn ui() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/ui/main/*.rs");
    }
}
