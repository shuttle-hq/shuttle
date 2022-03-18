use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let fn_decl = parse_macro_input!(item as ItemFn);

    let wrapper = create_wrapper(&fn_decl);
    let expanded = quote! {
        #wrapper

        #fn_decl
    };

    expanded.into()
}

fn create_wrapper(decl: &ItemFn) -> proc_macro2::TokenStream {
    let fn_iden = &decl.sig.ident;
    let return_type = &decl.sig.output;

    quote! {
        async fn wrapper(
            _factory: &mut dyn shuttle_service::Factory,
        ) #return_type {
            #fn_iden().await
        }

        shuttle_service::declare_service!(wrapper);
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;
    use quote::quote;
    use syn::parse_quote;

    use crate::create_wrapper;

    #[test]
    fn no_factory() {
        let input = parse_quote!(
            async fn simple() {}
        );
        let actual = create_wrapper(&input);
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
}
