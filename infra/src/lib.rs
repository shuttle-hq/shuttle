use std::collections::BTreeMap;

use proc_macro2::Span;
use syn::{parse_file, parse_quote, spanned::Spanned, Attribute, Item, ItemFn, LitStr, Meta, Path};

/// Takes rust source code and finds the `#[shuttle_runtime::main]`.
/// Then, parses the attribute meta of that function and returns a map of string->string or null.
pub fn parse_infra(rust_source_code: &str) -> Result<serde_json::Value, syn::Error> {
    let user_main = find_runtime_main_fn(rust_source_code)?;

    let Some((_main_fn, main_attr)) = user_main else {
        return Err(syn::Error::new(
            Span::call_site(),
            "No function using #[shuttle_runtime::main] found",
        ));
    };

    parse_infra_meta(&main_attr)
    // TODO: also parse user_main_fn argument attributes (resources) and add to IR
}

/// Parses rust source code and looks for a function annotated with `#[shuttle_runtime::main]`.
pub fn find_runtime_main_fn(
    rust_source_code: &str,
) -> Result<Option<(ItemFn, Attribute)>, syn::Error> {
    let ast = parse_file(rust_source_code)?;

    let runtime_main_path: Path = parse_quote! { shuttle_runtime::main };
    let main_fn_and_attr = ast.items.into_iter().find_map(|item| match item {
        Item::Fn(item_fn) => item_fn
            .attrs
            .clone()
            .into_iter()
            .find(|attr| attr.path() == &runtime_main_path)
            .map(|attr| (item_fn, attr)),
        _ => None,
    });

    Ok(main_fn_and_attr)
}

/// Parse the contents of `#[shuttle_runtime::main(...)]` as a list of `key = "value"` mappings.
/// TODO: support arbitrary nested Meta tree that maps into arbitrary Value.
fn parse_infra_meta(attr: &Attribute) -> Result<serde_json::Value, syn::Error> {
    let mut kv = BTreeMap::new();
    match attr.meta {
        // #[shuttle_runtime::main]
        Meta::Path(_) => Ok(serde_json::Value::Null),
        // #[shuttle_runtime::main(...)]
        Meta::List(ref meta_list) => {
            meta_list.parse_nested_meta(|meta| {
                let k = meta.path.require_ident()?.to_string();
                let v = meta.value()?.parse::<LitStr/* todo: allow more than strings */>()?.value();
                kv.insert(k, v);
                Ok(())
            })?;
            Ok(if kv.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::to_value(kv).unwrap()
            })
        }
        // ???
        Meta::NameValue(_) => Err(syn::Error::new(
            attr.span(),
            "Expected plain attribute or list",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infra_meta() {
        let attr: Attribute =
            parse_quote! { #[shuttle_runtime::main(instance_size = "m", replica_count = "2")] };
        assert_eq!(
            parse_infra_meta(&attr).unwrap(),
            serde_json::json!({"instance_size": "m", "replica_count": "2"})
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main(instance_size = "xyz",)] };
        assert_eq!(
            parse_infra_meta(&attr).unwrap(),
            serde_json::json!({"instance_size": "xyz"})
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main()] };
        assert_eq!(parse_infra_meta(&attr).unwrap(), serde_json::json!(null));

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main] };
        assert_eq!(parse_infra_meta(&attr).unwrap(), serde_json::json!(null));

        let attr: Attribute = parse_quote! { #[shuttle_runtime = "132"] };
        assert_eq!(
            parse_infra_meta(&attr).unwrap_err().to_string(),
            "Expected plain attribute or list"
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main(,)] };
        assert_eq!(
            parse_infra_meta(&attr).unwrap_err().to_string(),
            "unexpected token in nested attribute, expected ident"
        );
    }

    #[test]
    fn find_main_fn() {
        let rust = r#"
        use abc::def;

        fn blob() -> u8 {}

        #[shuttle_runtime::main]
        async fn main() -> ShuttleAxum {}
        "#;
        assert!(find_runtime_main_fn(rust).unwrap().is_some());

        // importing the main macro is not yet supported
        let rust = r#"
        use shuttle_runtime::main;
        #[main]
        async fn main() -> ShuttleAxum {}
        "#;
        assert!(find_runtime_main_fn(rust).unwrap().is_none());

        // must be in root of AST
        let rust = r#"
        mod not_root {
            #[shuttle_runtime::main]
            async fn main() -> ShuttleAxum {}
        }
        "#;
        assert!(find_runtime_main_fn(rust).unwrap().is_none());
    }

    #[test]
    fn parse() {
        let rust = r#"
        #[shuttle_runtime::main(
            this_thing = "great",
        )]
        async fn main() -> ShuttleAxum {}
        "#;
        let expected = serde_json::json!({"this_thing": "great"});
        let actual = parse_infra(rust).unwrap();
        assert_eq!(expected, actual);

        // only strings on RHS supported so far
        let rust = r#"
        #[shuttle_runtime::main(smoke = 420)]
        async fn main() -> ShuttleAxum {}
        "#;
        let expected = "expected string literal";
        let actual = parse_infra(rust).unwrap_err().to_string();
        assert_eq!(expected, actual);
    }
}
