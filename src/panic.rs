use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(location) = info.location() {
        let _ = crate::print!("PANIC at {}:{}", location.file(), location.line());
    }

    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
