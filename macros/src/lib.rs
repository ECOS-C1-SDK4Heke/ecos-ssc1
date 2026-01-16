extern crate proc_macro;

mod prelude;

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{ToTokens, quote};
use std::collections::HashMap;
use syn::{
    ItemFn, Meta, ReturnType, Token, parse::Parser, parse_macro_input, punctuated::Punctuated,
};

use crate::prelude::generate_prelude_imports;

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

    let docs: Vec<TokenStream2> = input_fn
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .map(|attr| {
            let attr_tokens = attr.to_token_stream();
            quote! { #attr_tokens }
        })
        .collect();

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

    let prelude = generate_prelude_imports();

    let dev_debug = if cfg!(feature = "dev") {
        quote! {
            loop {
                if '\n' as u8 == ecos_ssc1::Uart::read_byte_blocking() {
                    break;
                }
            }
        }
    } else {
        quote! {}
    };

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
        #prelude

        #(#docs)*
        #[unsafe(no_mangle)]
        pub extern "C" fn main() -> ! {
            #dev_debug

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
///     - no_gpio
///     - tick
///     - qspi || qspi(clkdiv=0) || qspi(0)
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

    let mut qspi_clkdiv: Option<u32> = None;

    for arg in &attr_args {
        match arg {
            Meta::Path(path) => {
                // 简单标识符，如 qspi
                if let Some(ident) = path.get_ident() {
                    if ident == "qspi" {
                        qspi_clkdiv = Some(0);
                    }
                }
            }
            Meta::List(list) => {
                // 带括号的参数，如 qspi(clkdiv=2) 或 qspi(2)
                if let Some(ident) = list.path.get_ident() {
                    if ident == "qspi" {
                        let args = parse_qspi_args(&list.tokens);
                        qspi_clkdiv = Some(args.clkdiv);
                    }
                }
            }
            Meta::NameValue(_) => {
                panic!("ecos_main does not support name=value syntax for qspi");
            }
        }
    }

    let docs: Vec<TokenStream2> = input_fn
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .map(|attr| {
            let attr_tokens = attr.to_token_stream();
            quote! { #attr_tokens }
        })
        .collect();

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
        if !cfg!(feature = "log") {
            quote! {
                unsafe {
                    ::ecos_ssc1::bindings::sys_uart_init();
                }
            }
        } else {
            quote! {}
        }
    });

    // 注册到on的：默认不会初始化（default_enabled = false），开启就得：xxx
    pm.register("tick", false, || {
        if !cfg!(feature = "log") {
            quote! {
                unsafe {
                    ::ecos_ssc1::bindings::sys_tick_init();
                }
            }
        } else {
            quote! {}
        }
    });

    pm.register("qspi", false, {
        let clkdiv = qspi_clkdiv;
        move || {
            if clkdiv.is_some() {
                let clkdiv_val = clkdiv.unwrap_or(0);
                quote! {
                    unsafe {
                        ::ecos_ssc1::bindings::qspi_init(::ecos_ssc1::bindings::qspi_config_t {
                            clkdiv: #clkdiv_val
                        });
                    }
                }
            } else {
                quote! {}
            }
        }
    });

    // 因为编译优化的原因，不调用一个函数对应的C就直接跳过了，导致其他函数找不到
    pm.register("gpio", true, || {
        quote! { unsafe { ::ecos_ssc1::bindings::gpio_config(
            // 16位全部输出
            &::ecos_ssc1::bindings::gpio_config_t {
                pin_bit_mask: 0xFFFF,
                mode: ::ecos_ssc1::bindings::gpio_mode_t_GPIO_MODE_OUTPUT,
            }
        ); } }
    });

    // on预设：开启所有注册到on的（默认不会初始化的）
    pm.add_preset("on", |pm| {
        // 有 on 标签就开启所有 default_enabled = false 的外设（注册到on的）
        pm.enable("tick");
        pm.enable("qspi");
    });

    // off预设：禁用所有注册到off的（默认会初始化的）
    pm.add_preset("off", |pm| {
        // 有 off 标签禁用所有 default_enabled = true 的外设（注册到off的）
        pm.disable("uart");
        pm.disable("gpio");
    });

    // ================ 处理传入的宏选项 ================
    for arg in attr_args {
        match arg {
            Meta::Path(path) => {
                if let Some(ident) = path.get_ident() {
                    let ident_str = ident.to_string();
                    // 跳过之前已处理的 qspi
                    if ident_str == "qspi" {
                        continue;
                    }

                    pm.process_option(&ident_str);
                }
            }
            Meta::List(list) => {
                // 只处理 qspi(...) 格式，其他列表格式不支持
                if let Some(ident) = list.path.get_ident() {
                    if ident != "qspi" {
                        panic!(
                            "ecos_main only supports qspi with parameters, other options must be simple identifiers"
                        );
                    }
                } else {
                    panic!(
                        "ecos_main only supports simple identifiers as options or qspi(...) syntax"
                    );
                }
            }
            Meta::NameValue(_) => {
                panic!("ecos_main does not support name=value syntax (except qspi(clkdiv=value))");
            }
        }
    }

    let init_pm = pm.generate_init_code();

    let dev_debug = if cfg!(feature = "dev") {
        quote! {
            loop {
                if '\n' as u8 == ecos_ssc1::Uart::read_byte_blocking() {
                    break;
                }
            }
        }
    } else {
        quote! {}
    };

    let init_log = if cfg!(feature = "log") {
        quote! {
            unsafe {
                // 启用log由于要打印时间戳以及初始化uart，附带开启tick ...
                ::ecos_ssc1::bindings::sys_uart_init();
                println!("asdsadas");
                ::ecos_ssc1::bindings::sys_tick_init();
                ::ecos_ssc1::features::log::init_logger();
            }
        }
    } else {
        quote! {}
    };

    let init_alloc = if cfg!(feature = "alloc") {
        quote! {
            unsafe {
                ::ecos_ssc1::features::alloc::init();
            }
        }
    } else {
        quote! {}
    };

    let prelude = generate_prelude_imports();

    let expanded = quote! {
        #prelude

        #(#docs)*
        #[unsafe(no_mangle)]
        pub extern "C" fn main() -> ! {
            #init_pm
            #init_log

            #dev_debug

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

fn parse_qspi_args(tokens: &TokenStream2) -> QspiArgs {
    let mut args = QspiArgs { clkdiv: 0 };

    if let Ok(value) = syn::parse::<syn::LitInt>(tokens.clone().into()) {
        args.clkdiv = value.base10_parse::<u32>().unwrap_or(0);
        return args;
    }

    let parser = Punctuated::<syn::Meta, Token![,]>::parse_terminated;
    if let Ok(meta_list) = parser.parse(tokens.clone().into()) {
        for meta in meta_list {
            match meta {
                syn::Meta::NameValue(nv) => {
                    if let Some(ident) = nv.path.get_ident() {
                        if ident == "clkdiv" {
                            if let syn::Expr::Lit(expr_lit) = &nv.value {
                                if let syn::Lit::Int(lit_int) = &expr_lit.lit {
                                    args.clkdiv = lit_int.base10_parse::<u32>().unwrap_or(0);
                                } else {
                                    panic!("clkdiv must be an integer literal");
                                }
                            } else {
                                panic!("clkdiv must be a literal");
                            }
                        } else {
                            panic!("qspi only supports clkdiv parameter");
                        }
                    }
                }
                _ => {
                    panic!("qspi only supports clkdiv=value syntax");
                }
            }
        }
        return args;
    }

    args
}

struct QspiArgs {
    clkdiv: u32,
}
