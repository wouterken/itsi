use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

#[proc_macro_attribute]
pub fn instrument_with_entry(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr_tokens = TokenStream2::from(attr);
    let input_fn = parse_macro_input!(item as ItemFn);
    let attrs = input_fn.attrs;
    let vis = input_fn.vis;
    let sig = input_fn.sig;
    let block = input_fn.block;
    let output = quote! {
        #[tracing::instrument(#attr_tokens)]
        #(#attrs)*
        #vis #sig {
            tracing::trace!("");
            #block
        }
    };

    output.into()
}
