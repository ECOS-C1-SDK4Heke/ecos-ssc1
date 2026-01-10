extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, Meta, Token, parse_macro_input, punctuated::Punctuated};

/// 直接执行版本：
///
/// #[rust_main]
/// fn app() -> ! { ... }
#[proc_macro_attribute]
pub fn rust_main(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    if !input_fn.sig.inputs.is_empty() {
        panic!("Function marked with #[rust_main] must have no parameters");
    }

    match &input_fn.sig.output {
        syn::ReturnType::Default => panic!("Function marked with #[rust_main] must return -> !"),
        syn::ReturnType::Type(_, ty) => {
            if let syn::Type::Never(_) = &**ty {
                // OK
            } else {
                panic!("Function marked with #[rust_main] must return -> !");
            }
        }
    }

    let fn_block = &input_fn.block;

    let expanded = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn main() -> ! {
            #fn_block
        }
    };

    TokenStream::from(expanded)
}

/// 默认初始化 UART / TIMER，
/// 可用 no_uart / no_timer 显式禁用：
///
/// #[ecos_main]
/// #[ecos_main(no_uart)]
/// #[ecos_main(no_timer)]
/// #[ecos_main(no_uart, no_timer)]
#[proc_macro_attribute]
pub fn ecos_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let metas = parse_macro_input!(
        attr with Punctuated::<Meta, Token![,]>::parse_terminated
    );
    let input_fn = parse_macro_input!(item as ItemFn);

    if !input_fn.sig.inputs.is_empty() {
        panic!("Function marked with #[ecos_main] must have no parameters");
    }

    match &input_fn.sig.output {
        syn::ReturnType::Default => panic!("Function marked with #[ecos_main] must return -> !"),
        syn::ReturnType::Type(_, ty) => {
            if let syn::Type::Never(_) = &**ty {
                // OK
            } else {
                panic!("Function marked with #[ecos_main] must return -> !");
            }
        }
    }

    let fn_block = &input_fn.block;

    let mut init_uart = true;
    let mut init_timer = true;

    for meta in metas {
        if let Meta::Path(path) = meta {
            if path.is_ident("no_uart") {
                init_uart = false;
            } else if path.is_ident("no_timer") {
                init_timer = false;
            } else {
                panic!(
                    "Unknown attribute: {:?}, expected `no_uart` or `no_timer`",
                    path.get_ident()
                );
            }
        } else {
            panic!("Expected path attribute like `no_uart` or `no_timer`");
        }
    }

    let uart_init = if init_uart {
        quote! {
            unsafe { ::ecos_ssc1::bindings::sys_uart_init(); }
        }
    } else {
        quote! {}
    };

    let timer_init = if init_timer {
        quote! {
            unsafe { ::ecos_ssc1::bindings::sys_tick_init(); }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn main() -> ! {
            #uart_init
            #timer_init
            #fn_block
        }
    };

    TokenStream::from(expanded)
}
