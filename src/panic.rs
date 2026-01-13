use core::panic::PanicInfo;

#[cfg(feature = "log")]
use crate::features::log::error as println;
#[cfg(not(feature = "log"))]
use crate::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    #[cfg(not(feature = "dev"))]
    {
        if let Some(location) = info.location() {
            let _ = println!(
                "PANIC at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
            let _ = println!("PANIC: {}", info.message());
        }

        loop {
            unsafe {
                core::arch::asm!("wfi");
            }
        }
    }
    #[cfg(feature = "dev")]
    {
        loop {
            println!("PANIC!!!");
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
    }
}
