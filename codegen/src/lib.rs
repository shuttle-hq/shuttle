use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, Ident, ItemFn, ReturnType};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_decl = parse_macro_input!(item as ItemFn);

    let wrapper = Wrapper::from_item_fn(&fn_decl);
    let expanded = quote! {
        #wrapper

        #fn_decl
    };

    expanded.into()
}

struct Wrapper {
    fn_ident: Ident,
    fn_output: ReturnType,
}

impl Wrapper {
    fn from_item_fn(item_fn: &ItemFn) -> Self {
        Self {
            fn_ident: item_fn.sig.ident.clone(),
            fn_output: item_fn.sig.output.clone(),
        }
    }
}

impl ToTokens for Wrapper {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let fn_output = &self.fn_output;
        let fn_ident = &self.fn_ident;

        let wrapper = quote! {
            async fn wrapper(
                _factory: &mut dyn shuttle_service::Factory,
            ) #fn_output {
                #fn_ident().await
            }

            shuttle_service::declare_service!(wrapper);
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
    }

    #[test]
    fn output_missing_return() {
        let input = Wrapper {
            fn_ident: parse_quote!(simple),
            fn_output: ReturnType::Default,
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn wrapper(
                _factory: &mut dyn shuttle_service::Factory,
            ) {
                simple().await
            }

            shuttle_service::declare_service!(wrapper);
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
    }

    #[test]
    fn output_with_return() {
        let input = Wrapper {
            fn_ident: parse_quote!(complex),
            fn_output: parse_quote!(-> Result<(), Box<dyn std::error::Error>>),
        };

        let actual = quote!(#input);
        let expected = quote! {
            async fn wrapper(
                _factory: &mut dyn shuttle_service::Factory,
            ) -> Result<(), Box<dyn std::error::Error> > {
                complex().await
            }

            shuttle_service::declare_service!(wrapper);
        };

        assert_eq!(actual.to_string(), expected.to_string());
    }
}
