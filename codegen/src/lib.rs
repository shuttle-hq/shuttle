#[cfg(feature = "next")]
mod next;
#[cfg(feature = "frameworks")]
mod shuttle_main;

#[cfg(feature = "frameworks")]
#[proc_macro_error::proc_macro_error]
#[proc_macro_attribute]
pub fn main(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    shuttle_main::r#impl(attr, item)
}

#[cfg(feature = "next")]
#[proc_macro_error::proc_macro_error]
#[proc_macro]
pub fn app(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    use next::App;
    use syn::{parse_macro_input, File};

    let mut file = parse_macro_input!(item as File);

    let app = App::from_file(&mut file);
    let bindings = next::wasi_bindings(app);

    quote::quote!(
        #file
        #bindings
    )
    .into()
}
