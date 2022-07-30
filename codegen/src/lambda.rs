use crate::resource::{self, Input};
use proc_macro::TokenStream;
use proc_macro_error::abort;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, FnArg, Ident, ItemFn};

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
    lambda_event_arg: FnArg,
}

impl Wrapper {
    /// Generates a wrapper that will act as the handler for the lambda.
    /// Non-input arguments will be bundled into a single Args struct as
    /// the expected payload for a lambda event.
    pub(super) fn wrap(item_fn: &mut ItemFn) -> Self {
        let mut args = item_fn.sig.inputs.iter_mut();

        // First argument for a lambda must be the LambdaEvent trigger
        let lambda_event_arg = args.next().cloned();

        let fn_inputs = resource::get_inputs(args);

        // Unwrap after getting resource inputs in order to report more errors before aborting
        let lambda_event_arg = lambda_event_arg.unwrap_or_else(|| {
            abort!(
                item_fn.sig.inputs, "Shuttle Lambda functions must have at least one argument";
                note = "The first parameter for a lambda must be a `lambda_runtime::LambdaEvent`"
            )
        });

        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_inputs,
            lambda_event_arg,
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
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::{parse_quote, FnArg, Ident};

    use super::{Input, Wrapper};

    #[test]
    fn from_with_inputs() {
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

        assert_eq!(actual.fn_ident, expected_ident);
        assert_eq!(actual.fn_inputs, expected_inputs);
        assert_eq!(actual.lambda_event_arg, expected_event_arg);
    }
}
