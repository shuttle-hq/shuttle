use proc_macro_error::emit_error;
use syn::{punctuated::IterMut, spanned::Spanned, Attribute, FnArg, Ident, Pat, Path};

#[derive(Debug, PartialEq)]
pub(crate) struct Input {
    /// The identifier for a resource input
    pub ident: Ident,

    /// The shuttle_service path to the builder for this resource
    pub builder: Path,
}

/// Returns the inputs that specify resources to provision
pub(crate) fn get_inputs(inputs: IterMut<FnArg>) -> Vec<Input> {
    inputs.filter_map(|input| match input {
                FnArg::Receiver(_) => None,
                FnArg::Typed(typed) => Some(typed),
            })
            .filter_map(|typed| match typed.pat.as_ref() {
                Pat::Ident(ident) => Some((ident, typed.attrs.drain(..).collect())),
                _ => None,
            })
            .filter_map(|(pat_ident, attrs)| {
                match attribute_to_path(attrs) {
                    Ok(builder) => Some(Input {
                        ident: pat_ident.ident.clone(),
                        builder,
                    }),
                    Err(err) => {
                        emit_error!(pat_ident, err; hint = pat_ident.span() => "Try adding a config like `#[shared::Postgres]`");
                        None
                    }
                }
            })
            .collect()
}

fn attribute_to_path(attrs: Vec<Attribute>) -> Result<Path, String> {
    if attrs.is_empty() {
        return Err("resource needs an attribute configuration".to_string());
    }

    let builder = attrs[0].path.clone();

    Ok(builder)
}
