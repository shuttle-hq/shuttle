use proc_macro::TokenStream;
use proc_macro_error::{emit_error, proc_macro_error};
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, Attribute, FnArg, Ident, ItemFn, Pat, Path,
    ReturnType, Stmt,
};

#[proc_macro_error]
#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut fn_decl = parse_macro_input!(item as ItemFn);

    let wrapper = Wrapper::from_item_fn(&mut fn_decl);

    let expanded = quote! {
        #wrapper

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
    fn_output: ReturnType,
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
    fn from_item_fn(item_fn: &mut ItemFn) -> Self {
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

        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_output: item_fn.sig.output.clone(),
            fn_inputs: inputs,
        }
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
        let fn_output = &self.fn_output;
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
                logger: Box<dyn shuttle_service::log::Log>,
            ) -> Result<Box<dyn shuttle_service::Service>, shuttle_service::Error> {
                #extra_imports

                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(logger)
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();


                #(let #fn_inputs = shuttle_service::#fn_inputs_builder::new().build(#factory_ident, runtime).await?;)*

                runtime.spawn(async {
                    #fn_ident(#(#fn_inputs),*).await.map(|ok| {
                        let r: Box<dyn shuttle_service::Service> = Box::new(ok);
                        r
                    })
                })
                .await
                .unwrap()
            }

            fn __binder(
                service: Box<dyn shuttle_service::Service>,
                addr: std::net::SocketAddr,
                runtime: &shuttle_service::Runtime,
            ) -> shuttle_service::ServeHandle {
                runtime.spawn(async move { service.bind(addr).await })
            }
        };

        wrapper.to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::{parse_quote, Ident, ReturnType};

    use crate::{Input, Wrapper};

    #[test]
    fn from_missing_return() {
        let mut input = parse_quote!(
            async fn simple() {}
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(simple);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, ReturnType::Default);
        assert_eq!(actual.fn_inputs, Vec::<Input>::new());
    }

    #[test]
    fn output_missing_return() {
        let input = Wrapper {
            fn_ident: parse_quote!(simple),
            fn_output: ReturnType::Default,
            fn_inputs: Vec::new(),
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                _factory: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: Box<dyn shuttle_service::log::Log>,
            ) {
                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(logger)
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                runtime.spawn(simple()).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_return() {
        let mut input = parse_quote!(
            async fn complex() -> Result<(), Box<dyn std::error::Error>> {}
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(complex);
        let expected_output: ReturnType = parse_quote!(-> Result<(), Box<dyn std::error::Error>>);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, expected_output);
        assert_eq!(actual.fn_inputs, Vec::<Input>::new());
    }

    #[test]
    fn output_with_return() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_output: parse_quote!(-> Result<(), Box<dyn std::error::Error>>),
            fn_inputs: Vec::new(),
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                _factory: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: Box<dyn shuttle_service::log::Log>,
            ) -> Result<(), Box<dyn std::error::Error> > {
                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(logger)
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                runtime.spawn(complex()).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_inputs() {
        let mut input = parse_quote!(
            async fn complex(
                #[shared::Postgres] pool: PgPool,
            ) -> Result<(), Box<dyn std::error::Error>> {
            }
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(complex);
        let expected_output: ReturnType = parse_quote!(-> Result<(), Box<dyn std::error::Error>>);
        let expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: parse_quote!(shared::Postgres),
        }];

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, expected_output);
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
            fn_output: parse_quote!(-> Result<(), Box<dyn std::error::Error>>),
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
            ) -> Result<(), Box<dyn std::error::Error> > {
                use shuttle_service::ResourceBuilder;

                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(logger)
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                let pool = shuttle_service::shared::Postgres::new().build(factory, runtime).await?;
                let redis = shuttle_service::shared::Redis::new().build(factory, runtime).await?;

                runtime.spawn(complex(pool, redis)).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn ui() {
        let t = trybuild::TestCases::new();
        t.compile_fail("tests/ui/*.rs");
    }
}
