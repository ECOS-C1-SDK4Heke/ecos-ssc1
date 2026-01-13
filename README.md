# ecos_ssc1

ECOS SDK Rust bindings for RISC-V bare metal development.

# Usage

```rust
#![no_std]
#![no_main]

use ecos_ssc1::ecos_main;

/// # Same As
///
/// ```rust
/// #![no_std]
/// #![no_main]
///
/// use ecos_ssc1::{rust_main, println, Uart::init as init_stdout};
///
/// #[rust_main]
/// fn emm() {
///     init_stdout();
///     println!("QwQ!");
/// }
/// ```
#[ecos_main]
fn xxx() {
    println!("QwQ!");
}
```

> 原则上，由于会自动扫ECOS_SDK_HOME环境变量下的C1的board目录以及通用的components和devices目录，所以C的驱动全部都可以自动集成

> todo-list：之后将基础的embedded-*全家桶适配，且可以使用features启用...
