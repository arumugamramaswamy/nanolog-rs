use nanolog_rs_common;
use proc_macro::{self, TokenStream};
use quote::quote;
use syn::parse_macro_input;

#[proc_macro]
pub fn nanolog(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as nanolog_rs_common::Nanolog);
    let i = quote::format_ident!("Log{}", input.get_log_type_suffix());
    let mut args = proc_macro2::TokenStream::new();
    for expr in input.punctuate {
        args.extend(quote! {#expr,});
    }
    let sink = input.sink;
    let tokens = quote! {
        {
            use crate::nanolog_internal::NanologLoggable;
            const L: u32 = line!();
            const F: u64 = ::nanolog_rs_common::const_fnv1a_hash(file!());
            let log = crate::nanolog_internal::#i::new(#args);
            <crate::nanolog_internal::#i as crate::nanolog_internal::NanologLoggable::<F, L>>::log(log, #sink);
        }
    };
    tokens.into()
}

// TODO: potentially make this an attribute macro of main
// TODO: this doesn't have to be a proc macro -> can probably get away with it being a regular
// macro
#[proc_macro]
pub fn setup_nanolog(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let tokens = quote! {
        mod nanolog_internal {
            include!(concat!(env!("OUT_DIR"), "/source_files.rs"));
        }
    };
    tokens.into()
}
