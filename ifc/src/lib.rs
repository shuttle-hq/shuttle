use proc_macro2::Span;
use shuttle_common::models::infra::InfraRequest;
use syn::{
    meta::{parser, ParseNestedMeta},
    parse::Parser,
    parse_file, parse_quote,
    spanned::Spanned,
    Attribute, Item, ItemFn, LitStr, Meta, MetaList, Path,
};

/// Takes rust source code and finds the `#[shuttle_runtime::main]`.
/// Then, parses the attribute meta of that function and returns a map of string->string or null.
pub fn parse_infra_from_code(rust_source_code: &str) -> Result<Option<InfraRequest>, syn::Error> {
    let Some((_main_fn, main_attr)) = find_runtime_main_fn(rust_source_code)? else {
        return Err(syn::Error::new(
            Span::call_site(),
            "No function using #[shuttle_runtime::main] found",
        ));
    };

    // TODO: also parse function argument attributes (resources) and add to IR (re-use parsing code from codegen)
    parse_infra_from_meta(&main_attr.meta)
}

/// Parses rust source code and looks for a function annotated with `#[shuttle_runtime::main]`.
pub fn find_runtime_main_fn(
    rust_source_code: &str,
) -> Result<Option<(ItemFn, Attribute)>, syn::Error> {
    let ast = parse_file(rust_source_code)?;

    let main_fn_and_attr = ast.items.into_iter().find_map(|item| match item {
        Item::Fn(item_fn) => main_fn_and_attr(item_fn),
        _ => None,
    });

    Ok(main_fn_and_attr)
}

/// Takes a function and return the function and the shuttle_runtime::main attribute
pub fn main_fn_and_attr(item_fn: ItemFn) -> Option<(ItemFn, Attribute)> {
    let runtime_main_path: Path = parse_quote! { shuttle_runtime::main };
    let codegen_main_path: Path = parse_quote! { shuttle_codegen::main };
    item_fn
        .attrs
        .clone()
        .into_iter()
        .find(|attr| attr.path() == &runtime_main_path || attr.path() == &codegen_main_path)
        .map(|attr| (item_fn, attr))
}

fn parse_infra_from_meta(meta: &Meta) -> Result<Option<InfraRequest>, syn::Error> {
    match meta {
        // #[shuttle_runtime::main]
        Meta::Path(_) => Ok(None),
        // #[shuttle_runtime::main(...)]
        Meta::List(ref meta_list) => parse_infra_from_meta_list(meta_list).map(Some),
        // #[shuttle_runtime = ...]
        Meta::NameValue(_) => Err(syn::Error::new(
            meta.span(),
            "Expected plain attribute or list",
        )),
    }
}

fn parse_infra_from_meta_list(meta_list: &MetaList) -> Result<InfraRequest, syn::Error> {
    let mut infra_parser = InfraAttrParser::default();
    let meta_parser = parser(|meta| infra_parser.parse_nested_meta(meta));
    meta_parser.parse2(meta_list.tokens.clone())?;
    Ok(infra_parser.into_infra())
}

#[derive(Default)]
pub struct InfraAttrParser(InfraRequest);
impl InfraAttrParser {
    /// Parses one argument provided to the `#[shuttle_runtime::main(...)]` attribute macro.
    ///
    /// Returns an error if the key or value could not be parsed into an expected value in [`InfraRequest`].
    pub fn parse_nested_meta(&mut self, meta: ParseNestedMeta) -> Result<(), syn::Error> {
        let key = meta.path.require_ident()?.to_string();
        let value = meta.value()?;
        match key.as_str() {
            "instance_size" => {
                self.0.instance_size =
                    Some(value.parse::<LitStr>()?.value().parse().map_err(|e| {
                        syn::Error::new(value.span(), format!("Invalid value: {e}"))
                    })?);
            }
            unknown_key => {
                return Err(syn::Error::new(
                    key.span(),
                    format!("Invalid macro attribute key: '{unknown_key}'"),
                ))
            }
        }
        Ok(())
    }
    pub fn into_infra(self) -> InfraRequest {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use shuttle_common::models::project::ComputeTier;

    use super::*;

    #[test]
    fn infra_meta() {
        let attr: Attribute = parse_quote! { #[shuttle_runtime::main(instance_size = "m")] };
        assert_eq!(
            parse_infra_from_meta(&attr.meta).unwrap().unwrap(),
            InfraRequest {
                instance_size: Some(ComputeTier::M),
                ..Default::default()
            }
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main(instance_size = "xyz",)] };
        assert_eq!(
            parse_infra_from_meta(&attr.meta).unwrap_err().to_string(),
            "Invalid value: Matching variant not found"
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main()] };
        assert_eq!(
            parse_infra_from_meta(&attr.meta).unwrap().unwrap(),
            InfraRequest::default()
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main] };
        assert_eq!(parse_infra_from_meta(&attr.meta).unwrap(), None);

        let attr: Attribute = parse_quote! { #[shuttle_runtime = "132"] };
        assert_eq!(
            parse_infra_from_meta(&attr.meta).unwrap_err().to_string(),
            "Expected plain attribute or list"
        );

        let attr: Attribute = parse_quote! { #[shuttle_runtime::main(,)] };
        assert_eq!(
            parse_infra_from_meta(&attr.meta).unwrap_err().to_string(),
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

        let rust = r#"
        #[shuttle_codegen::main]
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
            instance_size = "m",
        )]
        async fn main() -> ShuttleAxum {}
        "#;
        assert_eq!(
            parse_infra_from_code(rust).unwrap().unwrap(),
            InfraRequest {
                instance_size: Some(ComputeTier::M),
                ..Default::default()
            }
        );

        let rust = r#"
        #[shuttle_runtime::main { instance_size = "xxl" }      ]
        async fn main() -> ShuttleAxum {}
        "#;
        assert_eq!(
            parse_infra_from_code(rust).unwrap().unwrap(),
            InfraRequest {
                instance_size: Some(ComputeTier::XXL),
                ..Default::default()
            }
        );

        let rust = r#"
        #[shuttle_runtime::main[instance_size = "xs"]]
        async fn main() -> ShuttleAxum {}
        "#;
        assert_eq!(
            parse_infra_from_code(rust).unwrap().unwrap(),
            InfraRequest {
                instance_size: Some(ComputeTier::XS),
                ..Default::default()
            }
        );

        let rust = r#"
        #[shuttle_runtime::main(instance_size = 500000)]
        async fn main() -> ShuttleAxum {}
        "#;
        assert_eq!(
            parse_infra_from_code(rust).unwrap_err().to_string(),
            "expected string literal"
        );

        let rust = r#"
        #[shuttle_runtime::main(leet = 1337)]
        async fn main() -> ShuttleAxum {}
        "#;
        assert_eq!(
            parse_infra_from_code(rust).unwrap_err().to_string(),
            "Invalid macro attribute key: 'leet'"
        );
    }
}
