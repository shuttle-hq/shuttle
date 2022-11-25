mod main;
mod next;

use next::App;
use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use syn::{parse_macro_input, File};

#[proc_macro_error]
#[proc_macro_attribute]
pub fn main(attr: TokenStream, item: TokenStream) -> TokenStream {
    main::r#impl(attr, item)
}

#[proc_macro_error]
#[proc_macro]
pub fn app(item: TokenStream) -> TokenStream {
    let mut file = parse_macro_input!(item as File);

    let app = App::from_file(&mut file);

    quote::quote!(
        #file
        #app
    )
    .into()
}
