use crate::resource::{self, Input};
use proc_macro::TokenStream;
use proc_macro_error::emit_error;
use quote::{quote, ToTokens};
use syn::{
    parse_macro_input, parse_quote, spanned::Spanned, FnArg, Ident, ItemFn, Pat, PatIdent, PatType,
    ReturnType, Stmt,
};

const LAMBDA_EVENT_ARG_ERROR: &str =
    "The first parameter for a lambda must be a `lambda_runtime::LambdaEvent`";

pub(crate) fn r#impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut fn_decl = parse_macro_input!(item as ItemFn);
    let fn_name = fn_decl.sig.ident.clone();
    let runtime_name = format!("{fn_name}_lambda_runtime");

    let wrapper = Wrapper::wrap(&mut fn_decl);
    let wrapper_name = wrapper.ident();

    quote! {
        async fn #runtime_name() -> ::std::result::Result<(), ::shuttle_service::lambda_runtime::Error> {
            let func = ::shuttle_service::lambda_runtime::service_fn(#wrapper_name);
            ::shuttle_service::lambda_runtime::run(func).await?;
            Ok(())
        }

        #wrapper
    }
    .into()
}

struct Wrapper {
    fn_ident: Ident,
    fn_inputs: Vec<Input>,
    lambda_event_arg: Option<FnArg>,
    fn_return: ReturnType,
}

impl Wrapper {
    /// Generates a wrapper that will act as the handler for the lambda.
    /// Non-input arguments will be bundled into a single Args struct as
    /// the expected payload for a lambda event.
    pub(super) fn wrap(item_fn: &mut ItemFn) -> Self {
        let inputs_span = item_fn.sig.inputs.span();
        let mut args = item_fn.sig.inputs.iter_mut();

        // First argument for a lambda must be the LambdaEvent trigger
        let lambda_event_arg = args.next().cloned();
        if lambda_event_arg.is_none() {
            emit_error!(
                inputs_span, "Shuttle Lambda functions must have at least one argument";
                note = LAMBDA_EVENT_ARG_ERROR,
            );
        };

        let fn_inputs = resource::get_inputs(args);

        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_inputs,
            lambda_event_arg,
            fn_return: item_fn.sig.output.clone(),
        }
    }

    /// Returns identifier for wrapper function to pass to lambda runtime
    pub(super) fn ident(&self) -> Ident {
        let wrapped_name = self.fn_ident.to_string();
        Ident::new(&format!("__{wrapped_name}_wrapper"), self.fn_ident.span())
    }
}

impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let Self {
            fn_ident,
            lambda_event_arg,
            fn_return,
            ..
        } = self;
        let wrapper_name = self.ident();
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

        let lambda_event_arg_ident = lambda_event_arg.as_ref().map(arg_ident);
        if lambda_event_arg_ident.is_none() {
            emit_error!(lambda_event_arg, LAMBDA_EVENT_ARG_ERROR);
        }

        let wrapper = quote! {
            async fn #wrapper_name(#lambda_event_arg) #fn_return {
                #extra_imports

                #(let #fn_inputs = shuttle_service::#fn_inputs_builder::new().build(#factory_ident, runtime).await?;)*

                #fn_ident(#lambda_event_arg_ident, #(#fn_inputs),*)
            }
        };
        wrapper.to_tokens(tokens);
    }
}

fn arg_ident(lambda_event_arg: &FnArg) -> Option<Ident> {
    if let FnArg::Typed(PatType { pat, .. }) = lambda_event_arg {
        if let Pat::Ident(PatIdent { ident, .. }) = pat.as_ref() {
            Some(ident.clone())
        } else {
            None
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::{parse_quote, FnArg, Ident, ReturnType};

    use super::{Input, Wrapper};

    #[test]
    fn wrap_with_inputs() {
        let mut input = parse_quote!(
            async fn complex(
                name: LambdaEvent<String>,
                #[shared::Postgres] pool: PgPool,
            ) -> Result<Value, Error> {
            }
        );

        let actual = Wrapper::wrap(&mut input);

        let expected_ident: Ident = parse_quote!(complex);
        let expected_inputs: Vec<Input> = vec![Input {
            ident: parse_quote!(pool),
            builder: parse_quote!(shared::Postgres),
        }];
        let expected_event_arg: FnArg = parse_quote!(name: LambdaEvent<String>);
        let expected_return: ReturnType = parse_quote!(Result<Value, Error>);

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_inputs, expected_inputs);
        assert_eq!(actual.lambda_event_arg, Some(expected_event_arg));
        assert_eq!(actual.fn_return, expected_return);
    }

    #[test]
    fn output_with_inputs() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            lambda_event_arg: Some(parse_quote!(name: LambdaEvent<String>)),
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
            fn_return: parse_quote!(-> Result<Value, Error>),
        };

        let actual = quote!(#input);
        let expected = quote! {};

        println!(
            "{}",
            prettyplease::unparse(&syn::parse2(actual.clone()).unwrap())
        );

        assert_eq!(actual.to_string(), expected.to_string());
    }
}
