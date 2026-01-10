# ecos_ssc1

ECOS SDK Rust bindings for RISC-V bare metal development.

# Usage

```rust
#![no_std]
#![no_main]

use ecos_ssc1::{rust_main, println};

#[rust_main]  // 或者ecos_main
fn xxx() {
    println!("QwQ!");
}
```
