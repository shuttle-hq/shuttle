use std::collections::BTreeMap;

use proc_macro2::Span;
use syn::{parse_file, parse_quote, Attribute, Item, ItemFn, LitStr, Path};

/// Takes rust source code and looks for a function annotated with `#[shuttle_runtime::main]`.
/// Then, parses the `#[shuttle_infra(...)]` attribute of that function and returns a map of string->string, or null if the attribute is not there.
pub fn parse_infra(rust_source_code: &str) -> Result<serde_json::Value, syn::Error> {
    let user_main_fn = find_user_main_fn(rust_source_code)?;

    let Some(user_main_fn) = user_main_fn else {
        return Err(syn::Error::new(
            Span::call_site(),
            "No function using #[shuttle_runtime::main] found",
        ));
    };

    let infra_attr = user_main_fn
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("shuttle_infra"));

    let Some(infra_attr) = infra_attr else {
        return Ok(serde_json::Value::Null);
    };

    parse_infra_meta(infra_attr)
    // TODO: also parse user_main_fn argument attributes (resources) and add to IR
}

pub fn find_user_main_fn(rust_source_code: &str) -> Result<Option<ItemFn>, syn::Error> {
    let ast = parse_file(rust_source_code)?;

    let runtime_main_path: Path = parse_quote! { shuttle_runtime::main };
    let user_main_fn = ast.items.into_iter().find_map(|item| match item {
        Item::Fn(item_fn) => {
            if item_fn
                .attrs
                .iter()
                .any(|attr| attr.path() == &runtime_main_path)
            {
                Some(item_fn)
            } else {
                None
            }
        }
        _ => None,
    });

    Ok(user_main_fn)
}

/// Parse the contents of `#[shuttle_infra(...)]` as a list of `key = "value"` mappings.
/// TODO: support arbitrary nested Meta tree that maps into arbitrary Value.
fn parse_infra_meta(attr: &Attribute) -> Result<serde_json::Value, syn::Error> {
    let mut kv = BTreeMap::new();
    attr.meta.require_list()?.parse_nested_meta(|meta| {
        let k = meta.path.require_ident()?.to_string();
        let v = meta.value()?.parse::<LitStr/* todo: allow more than strings */>()?.value();
        kv.insert(k, v);
        Ok(())
    })?;
    let v = serde_json::to_value(kv).unwrap();
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infra_meta() {
        let attr: Attribute =
            parse_quote! { #[shuttle_infra(instance_size = "m", replica_count = "2")] };
        let expected = serde_json::json!({"instance_size": "m", "replica_count": "2"});
        let actual = parse_infra_meta(&attr).unwrap();
        assert_eq!(expected, actual);

        let attr: Attribute = parse_quote! { #[shuttle_infra(instance_size = "xyz",)] };
        let expected = serde_json::json!({"instance_size": "xyz"});
        let actual = parse_infra_meta(&attr).unwrap();
        assert_eq!(expected, actual);

        let attr: Attribute = parse_quote! { #[shuttle_infra()] };
        let expected = serde_json::json!({});
        let actual = parse_infra_meta(&attr).unwrap();
        assert_eq!(expected, actual);

        let attr: Attribute = parse_quote! { #[shuttle_infra] };
        parse_infra_meta(&attr).unwrap_err();
    }

    #[test]
    fn find_main_fn() {
        let rust = r#"
        use abc::def;

        fn blob() -> u8 {}

        #[shuttle_runtime::main]
        async fn main() -> ShuttleAxum {}
        "#;
        assert!(find_user_main_fn(&rust).unwrap().is_some());

        let rust = r#"
        use shuttle_runtime::main;
        #[main]
        async fn main() -> ShuttleAxum {}
        "#;
        assert!(find_user_main_fn(&rust).unwrap().is_none());
    }

    #[test]
    fn parse() {
        let rust = r#"
        #[shuttle_runtime::main]
        #[shuttle_infra(
            thing = "great",
        )]
        async fn main() -> ShuttleAxum {}
        "#;
        let expected = serde_json::json!({"thing": "great"});
        let actual = parse_infra(&rust).unwrap();
        assert_eq!(expected, actual);
    }
}
