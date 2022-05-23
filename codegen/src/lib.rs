use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, Attribute, FnArg, Ident, ItemFn, Pat, Path, ReturnType, Stmt,
};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut fn_decl = parse_macro_input!(item as ItemFn);

    let wrapper = Wrapper::from_item_fn(&mut fn_decl);
    let expanded = quote! {
        #wrapper

        #fn_decl

        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn shuttle_service::Service {
            // Ensure constructor returns concrete type.
            let constructor: for <'a> fn(
                &'a mut dyn shuttle_service::Factory,
                &'a shuttle_service::Runtime,
                shuttle_service::Logger,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = Result<_, shuttle_service::Error>> + Send + 'a>,
            > = |factory, runtime, logger| Box::pin(__shuttle_wrapper(factory, runtime, logger));

            let obj = shuttle_service::IntoService::into_service((constructor));
            let boxed: Box<dyn shuttle_service::Service> = Box::new(obj);
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
    ident: Ident,
    builder: Option<Path>,
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
            .map(|(pat_ident, attrs)| Input {
                ident: pat_ident.ident.clone(),
                builder: attribute_to_string(attrs),
            })
            .collect();

        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_output: item_fn.sig.output.clone(),
            fn_inputs: inputs,
        }
    }
}

fn attribute_to_string(attrs: Vec<Attribute>) -> Option<Path> {
    if attrs.is_empty() {
        return None;
    }

    let builder = attrs[0].path.clone();

    Some(builder)
}

impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let fn_output = &self.fn_output;
        let fn_ident = &self.fn_ident;
        let fn_inputs: Vec<_> = self.fn_inputs.iter().map(|i| i.ident.clone()).collect();
        let fn_inputs_attrs: Vec<_> = self.fn_inputs.iter().map(|i| i.builder.clone()).collect();

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
                logger: shuttle_service::Logger,
            ) #fn_output {
                #extra_imports

                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(Box::new(logger))
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();


                #(let #fn_inputs = shuttle_service::#fn_inputs_attrs::new().build(#factory_ident, runtime).await?;)*

                runtime.spawn(#fn_ident(#(#fn_inputs),*)).await.unwrap()
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
                logger: shuttle_service::Logger,
            ) {
                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(Box::new(logger))
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
                logger: shuttle_service::Logger,
            ) -> Result<(), Box<dyn std::error::Error> > {
                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(Box::new(logger))
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
            async fn complex(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {}
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(complex);
        let expected_output: ReturnType = parse_quote!(-> Result<(), Box<dyn std::error::Error>>);
        let expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: None,
        }];

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, expected_output);
        assert_eq!(actual.fn_inputs, expected_inputs);
    }

    #[test]
    fn output_with_inputs() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_output: parse_quote!(-> Result<(), Box<dyn std::error::Error>>),
            fn_inputs: vec![
                Input {
                    ident: parse_quote!(pool),
                    builder: None,
                },
                Input {
                    ident: parse_quote!(redis),
                    builder: None,
                },
            ],
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                factory: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: shuttle_service::Logger,
            ) -> Result<(), Box<dyn std::error::Error> > {
                use shuttle_service::ResourceBuilder;

                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(Box::new(logger))
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                let pool = factory.get_resource(runtime).await?;
                let redis = factory.get_resource(runtime).await?;

                runtime.spawn(complex(pool, redis)).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_input_attribute() {
        let mut input = parse_quote!(
            async fn meta(#[aws::rds] pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {}
        );

        let actual = Wrapper::from_item_fn(&mut input);
        let expected_ident: Ident = parse_quote!(meta);
        let expected_output: ReturnType = parse_quote!(-> Result<(), Box<dyn std::error::Error>>);
        let expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: Some(parse_quote!(aws::rds)),
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
    fn output_with_input_attributes() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_output: parse_quote!(-> Result<(), Box<dyn std::error::Error>>),
            fn_inputs: vec![Input {
                ident: parse_quote!(pool),
                builder: Some(parse_quote!(aws::rds)),
            }],
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                factory: &mut dyn shuttle_service::Factory,
                runtime: &shuttle_service::Runtime,
                logger: shuttle_service::Logger,
            ) -> Result<(), Box<dyn std::error::Error> > {
                use shuttle_service::ResourceBuilder;

                runtime.spawn_blocking(move || {
                    shuttle_service::log::set_boxed_logger(Box::new(logger))
                        .map(|()| shuttle_service::log::set_max_level(shuttle_service::log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                let pool = shuttle_service::aws::rds::new().build(factory, runtime).await?;

                runtime.spawn(complex(pool)).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }
}
