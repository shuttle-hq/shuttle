#[cfg(feature = "next")]
mod next;
#[cfg(feature = "frameworks")]
mod shuttle_main;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;

/// Setups up the `shuttle_runtime` to be executed. Allows
/// for user to setup a runtime without needing to explicitly
/// create one.
///
/// # Example
///
/// ```
/// [shuttle_runtime::main]
/// async fn entry_point() {
///     todo!();
/// }
/// ```
#[cfg(feature = "frameworks")]
#[proc_macro_error]
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    shuttle_main::r#impl(attr, item)
}

#[cfg(feature = "next")]
#[proc_macro_error]
#[proc_macro]
pub fn app(item: TokenStream) -> TokenStream {
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
