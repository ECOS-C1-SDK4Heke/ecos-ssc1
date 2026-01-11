extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::collections::HashMap;
use syn::{
    ItemFn, Meta, ReturnType, Token, parse::Parser, parse_macro_input, punctuated::Punctuated,
};

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

    let init_alloc = if cfg!(feature = "alloc") {
        quote! {
            unsafe {
                ::ecos_ssc1::features::alloc::init();
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn main() -> ! {
            #init_alloc
            #fn_block
        }
    };

    TokenStream::from(expanded)
}

/// 默认初始化 UART
///
/// 默认选项：
///     None
///
/// 可用选项：
///     - no_uart
///     - tick
///     - on == 一键开启all
///     - off == 一键关闭all == rust_main
///
/// #[ecos_main]
/// #[ecos_main(no_uart)]
/// #[ecos_main(tick)]
/// #[ecos_main(no_uart, tick, ...)]
#[proc_macro_attribute]
pub fn ecos_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let attr_args = parser.parse(attr).unwrap_or_default();

    let input_fn = parse_macro_input!(item as ItemFn);

    if !input_fn.sig.inputs.is_empty() {
        panic!("Function marked with #[ecos_main] must have no parameters");
    }

    match &input_fn.sig.output {
        ReturnType::Default => panic!("Function marked with #[ecos_main] must return -> !"),
        ReturnType::Type(_, ty) => {
            if let syn::Type::Never(_) = &**ty {
                // OK
            } else {
                panic!("Function marked with #[ecos_main] must return -> !");
            }
        }
    }

    let fn_block = &input_fn.block;

    let mut pm = PeripheralManager::new();

    // 注册到off的：默认会初始化（default_enabled = true），禁用就得：no_xxx
    pm.register("uart", true, || {
        quote! { unsafe { ::ecos_ssc1::bindings::sys_uart_init(); } }
    });

    // 注册到on的：默认不会初始化（default_enabled = false），开启就得：xxx
    pm.register("tick", false, || {
        quote! { unsafe { ::ecos_ssc1::bindings::sys_tick_init(); } }
    });

    // on预设：开启所有注册到on的（默认不会初始化的）
    pm.add_preset("on", |pm| {
        // 有 on 标签就开启所有 default_enabled = false 的外设（注册到on的）
        pm.enable("tick");
    });

    // off预设：禁用所有注册到off的（默认会初始化的）
    pm.add_preset("off", |pm| {
        // 有 off 标签禁用所有 default_enabled = true 的外设（注册到off的）
        pm.disable("uart");
    });

    // ================ 处理传入的宏选项 ================
    for arg in attr_args {
        match arg {
            Meta::Path(path) => {
                if let Some(ident) = path.get_ident() {
                    let ident_str = ident.to_string();
                    pm.process_option(&ident_str);
                }
            }
            Meta::List(_) | Meta::NameValue(_) => {
                panic!("ecos_main only supports simple identifiers as options");
            }
        }
    }

    let init_code = pm.generate_init_code();

    let init_alloc = if cfg!(feature = "alloc") {
        quote! {
            unsafe {
                ::ecos_ssc1::features::alloc::init();
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn main() -> ! {
            #init_code
            #init_alloc
            #fn_block
        }
    };

    TokenStream::from(expanded)
}

struct PeripheralManager {
    peripherals: HashMap<String, PeripheralConfig>,
    presets: HashMap<String, Box<dyn Fn(&mut PeripheralManager)>>,
}

impl PeripheralManager {
    fn new() -> Self {
        Self {
            peripherals: HashMap::new(),
            presets: HashMap::new(),
        }
    }

    /// 注册外设
    /// - name: 外设名称
    /// - default_enabled:
    ///   - true: 注册到off，默认会初始化
    ///   - false: 注册到on，默认不会初始化
    fn register<F>(&mut self, name: &str, default_enabled: bool, init_fn: F)
    where
        F: Fn() -> TokenStream2 + 'static,
    {
        self.peripherals.insert(
            name.to_string(),
            PeripheralConfig {
                enabled: default_enabled,
                init_fn: Box::new(init_fn),
            },
        );
    }

    fn add_preset<F>(&mut self, name: &str, preset_fn: F)
    where
        F: Fn(&mut PeripheralManager) + 'static,
    {
        self.presets.insert(name.to_string(), Box::new(preset_fn));
    }

    fn enable(&mut self, name: &str) {
        if let Some(config) = self.peripherals.get_mut(name) {
            config.enabled = true;
        }
    }

    fn disable(&mut self, name: &str) {
        if let Some(config) = self.peripherals.get_mut(name) {
            config.enabled = false;
        }
    }

    fn process_option(&mut self, option: &str) {
        match option {
            // 预设：on - 开启所有注册到on的（开启所有的默认不会初始化的）
            "on" => {
                let f = |pm: &mut PeripheralManager| {
                    pm.enable("tick");
                };
                f(self);
            }
            // 预设：off - 禁用所有注册到off的（关闭所有的默认会初始化的）
            "off" => {
                let f = |pm: &mut PeripheralManager| {
                    pm.disable("uart");
                };
                f(self);
            }
            _ => {
                // 检查是否是 no_xxx 格式（禁用默认初始化的）
                if let Some(periph_name) = option.strip_prefix("no_") {
                    // 禁用注册到off的外设（默认会初始化的）
                    self.disable(periph_name);
                } else {
                    // 否则是开启注册到on的外设（默认不会初始化的）
                    self.enable(option);
                }
            }
        }
    }

    fn generate_init_code(&self) -> TokenStream2 {
        let mut code = TokenStream2::new();

        for (_, config) in &self.peripherals {
            if config.enabled {
                let init_code = (config.init_fn)();
                code.extend(init_code);
            }
        }

        code
    }
}

struct PeripheralConfig {
    enabled: bool,
    init_fn: Box<dyn Fn() -> TokenStream2>,
}
