//! todo: 以后增加更多的，比如支持trace back信息显示的...

use core::panic::PanicInfo;

mod basic;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    basic::panic(info);
}
