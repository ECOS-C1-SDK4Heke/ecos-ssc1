use core::fmt;

use crate::bindings;

pub struct Uart;

impl Uart {
    pub fn init() {
        unsafe {
            crate::bindings::sys_uart_init();
        }
    }

    pub fn write_byte(b: u8) {
        unsafe {
            crate::bindings::sys_putchar(b.into());
        }
    }

    pub fn write_str(s: &str) {
        for b in s.bytes() {
            Self::write_byte(b);
        }
    }

    pub fn read_byte_nonblock() -> Option<u8> {
        unsafe {
            let reg = core::ptr::read_volatile(bindings::REG_UART_0_DATA as *const i32);
            if reg != -1 { Some(reg as u8) } else { None }
        }
    }

    pub fn read_byte_blocking() -> u8 {
        loop {
            if let Some(b) = Self::read_byte_nonblock() {
                return b;
            }
        }
    }

    pub fn write_bytes(bytes: &[u8]) {
        for &b in bytes {
            Self::write_byte(b);
        }
    }
}

pub struct UartWriter;

impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Uart::write_str(s);
        Ok(())
    }
}
