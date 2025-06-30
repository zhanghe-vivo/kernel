extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, ItemFn};

#[proc_macro_attribute]
pub fn test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let test_name = &input.sig.ident;
    let input_block = &input.block;

    let filtered_params = input
        .sig
        .inputs
        .iter()
        .filter(|arg| !matches!(arg, FnArg::Receiver(_)));

    let param_names = filtered_params.clone().filter_map(|arg| match arg {
        FnArg::Typed(pat_type) => Some(&pat_type.pat),
        _ => None,
    });

    let expanded = quote! {
        #[allow(dead_code)]
        #[test_case]
        fn #test_name(#(#filtered_params),*) {
            use semihosting::println;
            println!("[ RUN      ] {}", stringify!(#test_name));
            #( let _ = #param_names; )*
            #input_block
            println!("[       OK ] {}", stringify!(#test_name));
        }
    };
    expanded.into()
}
