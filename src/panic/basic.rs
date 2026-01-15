use core::panic::PanicInfo;

#[cfg(feature = "log")]
use crate::features::log::error as println;
#[cfg(not(feature = "log"))]
use crate::println;

pub(super) fn log_panic(info: &PanicInfo) {
    if let Some(location) = info.location() {
        let _ = println!(
            "PANIC at {}:{}:{}",
            location.file(),
            location.line(),
            location.column()
        );
        let _ = println!("PANIC: {}", info.message());
    }
}

pub(super) fn panic(info: &PanicInfo) -> ! {
    {
        loop {
            log_panic(info);

            #[cfg(not(feature = "dev"))]
            break;
        }

        loop {
            unsafe {
                core::arch::asm!("wfi");
            }
        }
    }
}
