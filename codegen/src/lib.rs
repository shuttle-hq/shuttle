use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, parse_quote, FnArg, Ident, ItemFn, Pat, ReturnType, Stmt};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_decl = parse_macro_input!(item as ItemFn);

    let wrapper = Wrapper::from_item_fn(&fn_decl);
    let expanded = quote! {
        #wrapper

        #fn_decl

        #[no_mangle]
        pub extern "C" fn _create_service() -> *mut dyn shuttle_service::Service {
            // Ensure constructor returns concrete type.
            let constructor: for <'a> fn(
                &'a mut dyn shuttle_service::Factory,
                &'a tokio::runtime::Runtime,
                shuttle_service::logger::Logger,
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
    fn_inputs: Vec<Ident>,
}

impl Wrapper {
    fn from_item_fn(item_fn: &ItemFn) -> Self {
        let inputs: Vec<_> = item_fn
            .sig
            .inputs
            .iter()
            .filter_map(|input| match input {
                FnArg::Receiver(_) => None,
                FnArg::Typed(typed) => Some(typed),
            })
            .filter_map(|typed| match typed.pat.as_ref() {
                Pat::Ident(ident) => Some(ident),
                _ => None,
            })
            .map(|pat_ident| pat_ident.ident.clone())
            .collect();

        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_output: item_fn.sig.output.clone(),
            fn_inputs: inputs,
        }
    }
}

impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let fn_output = &self.fn_output;
        let fn_ident = &self.fn_ident;
        let fn_inputs = &self.fn_inputs;

        let factory_ident: Ident = if self.fn_inputs.is_empty() {
            parse_quote!(_factory)
        } else {
            parse_quote!(factory)
        };

        let extra_imports: Option<Stmt> = if self.fn_inputs.is_empty() {
            None
        } else {
            Some(parse_quote!(
                use shuttle_service::GetResource;
            ))
        };

        let wrapper = quote! {
            async fn __shuttle_wrapper(
                #factory_ident: &mut dyn shuttle_service::Factory,
                runtime: &tokio::runtime::Runtime,
                logger: shuttle_service::logger::Logger,
            ) #fn_output {
                #extra_imports

                runtime.spawn(async {
                    log::set_boxed_logger(Box::new(logger))
                        .map(|()| log::set_max_level(log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();


                #(let #fn_inputs = #factory_ident.get_resource(runtime).await?;)*

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

    use crate::Wrapper;

    #[test]
    fn from_missing_return() {
        let input = parse_quote!(
            async fn simple() {}
        );

        let actual = Wrapper::from_item_fn(&input);
        let expected_ident: Ident = parse_quote!(simple);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, ReturnType::Default);
        assert_eq!(actual.fn_inputs, Vec::<Ident>::new());
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
                runtime: &tokio::runtime::Runtime,
                logger: shuttle_service::logger::Logger,
            ) {
                runtime.spawn(async {
                    log::set_boxed_logger(Box::new(logger))
                        .map(|()| log::set_max_level(log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                runtime.spawn(simple()).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_return() {
        let input = parse_quote!(
            async fn complex() -> Result<(), Box<dyn std::error::Error>> {}
        );

        let actual = Wrapper::from_item_fn(&input);
        let expected_ident: Ident = parse_quote!(complex);
        let expected_output: ReturnType = parse_quote!(-> Result<(), Box<dyn std::error::Error>>);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, expected_output);
        assert_eq!(actual.fn_inputs, Vec::<Ident>::new());
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
                runtime: &tokio::runtime::Runtime,
                logger: shuttle_service::logger::Logger,
            ) -> Result<(), Box<dyn std::error::Error> > {
                runtime.spawn(async {
                    log::set_boxed_logger(Box::new(logger))
                        .map(|()| log::set_max_level(log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                runtime.spawn(complex()).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_inputs() {
        let input = parse_quote!(
            async fn complex(pool: PgPool) -> Result<(), Box<dyn std::error::Error>> {}
        );

        let actual = Wrapper::from_item_fn(&input);
        let expected_ident: Ident = parse_quote!(complex);
        let expected_output: ReturnType = parse_quote!(-> Result<(), Box<dyn std::error::Error>>);
        let expected_inputs: Vec<Ident> = vec![parse_quote!(pool)];

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_output, expected_output);
        assert_eq!(actual.fn_inputs, expected_inputs);
    }

    #[test]
    fn output_with_inputs() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_output: parse_quote!(-> Result<(), Box<dyn std::error::Error>>),
            fn_inputs: vec![parse_quote!(pool), parse_quote!(redis)],
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn __shuttle_wrapper(
                factory: &mut dyn shuttle_service::Factory,
                runtime: &tokio::runtime::Runtime,
                logger: shuttle_service::logger::Logger,
            ) -> Result<(), Box<dyn std::error::Error> > {
                use shuttle_service::GetResource;

                runtime.spawn(async {
                    log::set_boxed_logger(Box::new(logger))
                        .map(|()| log::set_max_level(log::LevelFilter::Info))
                        .expect("logger set should succeed");
                }).await.unwrap();

                let pool = factory.get_resource(runtime).await?;
                let redis = factory.get_resource(runtime).await?;

                runtime.spawn(complex(pool, redis)).await.unwrap()
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }
}
