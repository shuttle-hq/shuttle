use proc_macro::TokenStream;
use proc_macro_error::emit_error;
use quote::{quote, ToTokens};
use syn::{
    parenthesized, parse::Parse, parse2, parse_macro_input, parse_quote, punctuated::Punctuated,
    spanned::Spanned, token::Paren, Attribute, Expr, FnArg, Ident, ItemFn, Pat, PatIdent, Path,
    ReturnType, Signature, Stmt, Token, Type, TypePath,
};

pub(crate) fn r#impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut fn_decl = parse_macro_input!(item as ItemFn);

    let loader = Loader::from_item_fn(&mut fn_decl);

    let expanded = quote! {
        #[tokio::main]
        async fn main() {
            shuttle_runtime::start(loader).await;
        }

        #loader

        #fn_decl
    };

    expanded.into()
}

struct Loader {
    fn_ident: Ident,
    fn_inputs: Vec<Input>,
    fn_return: TypePath,
}

#[derive(Debug, PartialEq)]
struct Input {
    /// The identifier for a resource input
    ident: Ident,

    /// The shuttle_service builder for this resource
    builder: Builder,
}

#[derive(Debug, PartialEq)]
struct Builder {
    /// Path to the builder
    path: Path,

    /// Options to call on the builder
    options: BuilderOptions,
}

#[derive(Debug, Default, PartialEq)]
struct BuilderOptions {
    /// Parenthesize around options
    paren_token: Paren,

    /// The actual options
    options: Punctuated<BuilderOption, Token![,]>,
}

#[derive(Debug, PartialEq)]
struct BuilderOption {
    /// Identifier of the option to set
    ident: Ident,

    /// Value to set option to
    value: Expr,
}

impl Parse for BuilderOptions {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;

        Ok(Self {
            paren_token: parenthesized!(content in input),
            options: content.parse_terminated(BuilderOption::parse)?,
        })
    }
}

impl ToTokens for BuilderOptions {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let (methods, values): (Vec<_>, Vec<_>) =
            self.options.iter().map(|o| (&o.ident, &o.value)).unzip();
        let chain = quote!(#(.#methods(#values))*);

        chain.to_tokens(tokens);
    }
}

impl Parse for BuilderOption {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident = input.parse()?;
        let _equal: Token![=] = input.parse()?;
        let value = input.parse()?;

        Ok(Self { ident, value })
    }
}

impl Loader {
    pub(crate) fn from_item_fn(item_fn: &mut ItemFn) -> Option<Self> {
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
                match attribute_to_builder(pat_ident, attrs) {
                    Ok(builder) => Some(Input {
                        ident: pat_ident.ident.clone(),
                        builder,
                    }),
                    Err(err) => {
                        emit_error!(pat_ident, err; hint = pat_ident.span() => "Try adding a config like `#[shuttle_shared_db::Postgres]`");
                        None
                    }
                }
            })
            .collect();

        if let Some(type_path) = check_return_type(item_fn.sig.clone()) {
            Some(Self {
                fn_ident: item_fn.sig.ident.clone(),
                fn_inputs: inputs,
                fn_return: type_path,
            })
        } else {
            None
        }
    }
}

fn check_return_type(signature: Signature) -> Option<TypePath> {
    match signature.output {
        ReturnType::Default => {
            emit_error!(
                signature,
                "shuttle_service::main functions need to return a service";
                hint = "See the docs for services with first class support";
                doc = "https://docs.rs/shuttle-service/latest/shuttle_service/attr.main.html#shuttle-supported-services"
            );
            None
        }
        ReturnType::Type(_, r#type) => match *r#type {
            Type::Path(path) => Some(path),
            _ => {
                emit_error!(
                    r#type,
                    "shuttle_service::main functions need to return a first class service or 'Result<impl Service, shuttle_service::Error>";
                    hint = "See the docs for services with first class support";
                    doc = "https://docs.rs/shuttle-service/latest/shuttle_service/attr.main.html#shuttle-supported-services"
                );
                None
            }
        },
    }
}

fn attribute_to_builder(pat_ident: &PatIdent, attrs: Vec<Attribute>) -> syn::Result<Builder> {
    if attrs.is_empty() {
        return Err(syn::Error::new_spanned(
            pat_ident,
            "resource needs an attribute configuration",
        ));
    }

    let options = if attrs[0].tokens.is_empty() {
        Default::default()
    } else {
        parse2(attrs[0].tokens.clone())?
    };

    let builder = Builder {
        path: attrs[0].path.clone(),
        options,
    };

    Ok(builder)
}

impl ToTokens for Loader {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let fn_ident = &self.fn_ident;

        let return_type = &self.fn_return;

        let mut fn_inputs: Vec<_> = Vec::with_capacity(self.fn_inputs.len());
        let mut fn_inputs_builder: Vec<_> = Vec::with_capacity(self.fn_inputs.len());
        let mut fn_inputs_builder_options: Vec<_> = Vec::with_capacity(self.fn_inputs.len());

        for input in self.fn_inputs.iter() {
            fn_inputs.push(&input.ident);
            fn_inputs_builder.push(&input.builder.path);
            fn_inputs_builder_options.push(&input.builder.options);
        }

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

        let loader = quote! {
            async fn loader<S: shuttle_service::StorageManager>(
                mut #factory_ident: shuttle_runtime::ProvisionerFactory<S>,
            ) -> #return_type {
                use shuttle_service::Context;
                #extra_imports

                #(let #fn_inputs = #fn_inputs_builder::new()#fn_inputs_builder_options.build(&mut #factory_ident).await.context(format!("failed to provision {}", stringify!(#fn_inputs_builder)))?;)*

                #fn_ident(#(#fn_inputs),*).await
            }
        };

        loader.to_tokens(tokens);
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::{parse_quote, Ident};

    use super::{Builder, BuilderOptions, Input, Loader};

    #[test]
    fn from_with_return() {
        let mut input = parse_quote!(
            async fn simple() -> ShuttleAxum {}
        );

        let actual = Loader::from_item_fn(&mut input).unwrap();
        let expected_ident: Ident = parse_quote!(simple);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_inputs, Vec::<Input>::new());
    }

    #[test]
    fn output_with_return() {
        let input = Loader {
            fn_ident: parse_quote!(simple),
            fn_inputs: Vec::new(),
            fn_return: parse_quote!(ShuttleSimple),
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn loader<S: shuttle_service::StorageManager>(
                mut _factory: shuttle_runtime::ProvisionerFactory<S>,
            ) -> ShuttleSimple {
                use shuttle_service::Context;
                simple().await
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_inputs() {
        let mut input = parse_quote!(
            async fn complex(#[shuttle_shared_db::Postgres] pool: PgPool) -> ShuttleTide {}
        );

        let actual = Loader::from_item_fn(&mut input).unwrap();
        let expected_ident: Ident = parse_quote!(complex);
        let expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: Builder {
                path: parse_quote!(shuttle_shared_db::Postgres),
                options: Default::default(),
            },
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
        let input = Loader {
            fn_ident: parse_quote!(complex),
            fn_inputs: vec![
                Input {
                    ident: parse_quote!(pool),
                    builder: Builder {
                        path: parse_quote!(shuttle_shared_db::Postgres),
                        options: Default::default(),
                    },
                },
                Input {
                    ident: parse_quote!(redis),
                    builder: Builder {
                        path: parse_quote!(shuttle_shared_db::Redis),
                        options: Default::default(),
                    },
                },
            ],
            fn_return: parse_quote!(ShuttleComplex),
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn loader<S: shuttle_service::StorageManager>(
                mut factory: shuttle_runtime::ProvisionerFactory<S>,
            ) -> ShuttleComplex {
                use shuttle_service::Context;
                use shuttle_service::ResourceBuilder;

                let pool = shuttle_shared_db::Postgres::new().build(&mut factory).await.context(format!("failed to provision {}", stringify!(shuttle_shared_db::Postgres)))?;
                let redis = shuttle_shared_db::Redis::new().build(&mut factory).await.context(format!("failed to provision {}", stringify!(shuttle_shared_db::Redis)))?;

                complex(pool, redis).await
            }
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn parse_builder_options() {
        let input: BuilderOptions = parse_quote!((
            string = "string_val",
            boolean = true,
            integer = 5,
            float = 2.65,
            enum_variant = SomeEnum::Variant1
        ));

        let mut expected: BuilderOptions = Default::default();
        expected.options.push(parse_quote!(string = "string_val"));
        expected.options.push(parse_quote!(boolean = true));
        expected.options.push(parse_quote!(integer = 5));
        expected.options.push(parse_quote!(float = 2.65));
        expected
            .options
            .push(parse_quote!(enum_variant = SomeEnum::Variant1));

        assert_eq!(input, expected);
    }

    #[test]
    fn tokenize_builder_options() {
        let mut input: BuilderOptions = Default::default();
        input.options.push(parse_quote!(string = "string_val"));
        input.options.push(parse_quote!(boolean = true));
        input.options.push(parse_quote!(integer = 5));
        input.options.push(parse_quote!(float = 2.65));
        input
            .options
            .push(parse_quote!(enum_variant = SomeEnum::Variant1));

        let actual = quote!(#input);
        let expected = quote!(.string("string_val").boolean(true).integer(5).float(2.65).enum_variant(SomeEnum::Variant1));

        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn from_with_input_options() {
        let mut input = parse_quote!(
            async fn complex(
                #[shared::Postgres(size = "10Gb", public = false)] pool: PgPool,
            ) -> ShuttlePoem {
            }
        );

        let actual = Loader::from_item_fn(&mut input).unwrap();
        let expected_ident: Ident = parse_quote!(complex);
        let mut expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: Builder {
                path: parse_quote!(shared::Postgres),
                options: Default::default(),
            },
        }];

        expected_inputs[0]
            .builder
            .options
            .options
            .push(parse_quote!(size = "10Gb"));
        expected_inputs[0]
            .builder
            .options
            .options
            .push(parse_quote!(public = false));

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_inputs, expected_inputs);
    }

    #[test]
    fn output_with_input_options() {
        let mut input = Loader {
            fn_ident: parse_quote!(complex),
            fn_inputs: vec![Input {
                ident: parse_quote!(pool),
                builder: Builder {
                    path: parse_quote!(shuttle_shared_db::Postgres),
                    options: Default::default(),
                },
            }],
            fn_return: parse_quote!(ShuttleComplex),
        };

        input.fn_inputs[0]
            .builder
            .options
            .options
            .push(parse_quote!(size = "10Gb"));
        input.fn_inputs[0]
            .builder
            .options
            .options
            .push(parse_quote!(public = false));

        let actual = quote!(#input);
        let expected = quote! {
            async fn loader<S: shuttle_service::StorageManager>(
                mut factory: shuttle_runtime::ProvisionerFactory<S>,
            ) -> ShuttleComplex {
                use shuttle_service::Context;
                use shuttle_service::ResourceBuilder;

                let pool = shuttle_shared_db::Postgres::new().size("10Gb").public(false).build(&mut factory).await.context(format!("failed to provision {}", stringify!(shuttle_shared_db::Postgres)))?;

                complex(pool).await
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
