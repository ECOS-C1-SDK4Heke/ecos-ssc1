#![no_std]

pub mod gpio;
pub mod panic;
pub mod timer;
pub mod uart;

pub use macros::{ecos_main, rust_main};

pub use crate::{gpio::Gpio, gpio::GpioPin, timer::Timer, uart::Uart};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = write!($crate::uart::UartWriter, $($arg)*);
    }};
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[allow(nonstandard_style)]
pub mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

    // board.h i2c_type.h 中 bindgen 识别不能的字段重映射

    /* ========== GPIO 寄存器组 ========== */
    pub const REG_GPIO_0_DR: *mut volatile::VolatilePtr<u32> =
        0x03000000 as *mut volatile::VolatilePtr<u32>;
    pub const REG_GPIO_0_DDR: *mut volatile::VolatilePtr<u32> =
        0x03000004 as *mut volatile::VolatilePtr<u32>;
    pub const REG_GPIO_0_PUB: *mut volatile::VolatilePtr<u32> =
        0x03000008 as *mut volatile::VolatilePtr<u32>;
    pub const REG_GPIO_0_PDB: *mut volatile::VolatilePtr<u32> =
        0x0300000c as *mut volatile::VolatilePtr<u32>;

    /* ========== HP_UART 寄存器组 ======= */
    pub const REG_UART_1_LCR: *mut volatile::VolatilePtr<u32> =
        0x03003000 as *mut volatile::VolatilePtr<u32>;
    pub const REG_UART_1_DIV: *mut volatile::VolatilePtr<u32> =
        0x03003004 as *mut volatile::VolatilePtr<u32>;
    pub const REG_UART_1_TRX: *mut volatile::VolatilePtr<u32> =
        0x03003008 as *mut volatile::VolatilePtr<u32>;
    pub const REG_UART_1_FCR: *mut volatile::VolatilePtr<u32> =
        0x0300300c as *mut volatile::VolatilePtr<u32>;
    pub const REG_UART_1_LSR: *mut volatile::VolatilePtr<u32> =
        0x03003010 as *mut volatile::VolatilePtr<u32>;

    /* ========== I2C 接口寄存器 ========== */
    pub const REG_I2C_0_CTRL: *mut volatile::VolatilePtr<u32> =
        0x03006000 as *mut volatile::VolatilePtr<u32>;
    pub const REG_I2C_0_PSCR: *mut volatile::VolatilePtr<u32> =
        0x03006004 as *mut volatile::VolatilePtr<u32>;
    pub const REG_I2C_0_TXR: *mut volatile::VolatilePtr<u32> =
        0x03006008 as *mut volatile::VolatilePtr<u32>;
    pub const REG_I2C_0_RXR: *mut volatile::VolatilePtr<u32> =
        0x0300600c as *mut volatile::VolatilePtr<u32>;
    pub const REG_I2C_0_CMD: *mut volatile::VolatilePtr<u32> =
        0x03006010 as *mut volatile::VolatilePtr<u32>;
    pub const REG_I2C_0_SR: *mut volatile::VolatilePtr<u32> =
        0x03006014 as *mut volatile::VolatilePtr<u32>;

    /* ========== PWM 寄存器组 ========== */
    pub const REG_PWM_0_CTRL: *mut volatile::VolatilePtr<u32> =
        0x03004000 as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_PSCR: *mut volatile::VolatilePtr<u32> =
        0x03004004 as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_CNT: *mut volatile::VolatilePtr<u32> =
        0x03004008 as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_CMP: *mut volatile::VolatilePtr<u32> =
        0x0300400c as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_CR0: *mut volatile::VolatilePtr<u32> =
        0x03004010 as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_CR1: *mut volatile::VolatilePtr<u32> =
        0x03004014 as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_CR2: *mut volatile::VolatilePtr<u32> =
        0x03004018 as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_CR3: *mut volatile::VolatilePtr<u32> =
        0x0300401c as *mut volatile::VolatilePtr<u32>;
    pub const REG_PWM_0_STAT: *mut volatile::VolatilePtr<u32> =
        0x03004020 as *mut volatile::VolatilePtr<u32>;

    /* ========== QSPI 接口寄存器 ========== */
    pub const REG_QSPI_0_STATUS: *mut volatile::VolatilePtr<u32> =
        0x03007000 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_CLKDIV: *mut volatile::VolatilePtr<u32> =
        0x03007004 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_CMD: *mut volatile::VolatilePtr<u32> =
        0x03007008 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_ADR: *mut volatile::VolatilePtr<u32> =
        0x0300700c as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_LEN: *mut volatile::VolatilePtr<u32> =
        0x03007010 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_DUM: *mut volatile::VolatilePtr<u32> =
        0x03007014 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_TXFIFO: *mut volatile::VolatilePtr<u32> =
        0x03007018 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_RXFIFO: *mut volatile::VolatilePtr<u32> =
        0x03007020 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_INTCFG: *mut volatile::VolatilePtr<u32> =
        0x03007024 as *mut volatile::VolatilePtr<u32>;
    pub const REG_QSPI_0_INTSTA: *mut volatile::VolatilePtr<u32> =
        0x03007028 as *mut volatile::VolatilePtr<u32>;

    /* ========== SYS_UART 接口寄存器 ====== */
    pub const REG_UART_0_CLKDIV: *mut volatile::VolatilePtr<u32> =
        0x03000010 as *mut volatile::VolatilePtr<u32>;
    pub const REG_UART_0_DATA: *mut volatile::VolatilePtr<u32> =
        0x03000014 as *mut volatile::VolatilePtr<u32>;

    /* ========== 定时器 寄存器组 =========== */
    pub const REG_TIM0_CONFIG: *mut volatile::VolatilePtr<u32> =
        0x0300005c as *mut volatile::VolatilePtr<u32>;
    pub const REG_TIM0_VALUE: *mut volatile::VolatilePtr<u32> =
        0x03000060 as *mut volatile::VolatilePtr<u32>;
    pub const REG_TIM0_DATA: *mut volatile::VolatilePtr<u32> =
        0x03000064 as *mut volatile::VolatilePtr<u32>;

    pub const REG_TIM1_CONFIG: *mut volatile::VolatilePtr<u32> =
        0x03000068 as *mut volatile::VolatilePtr<u32>;
    pub const REG_TIM1_VALUE: *mut volatile::VolatilePtr<u32> =
        0x0300006c as *mut volatile::VolatilePtr<u32>;
    pub const REG_TIM1_DATA: *mut volatile::VolatilePtr<u32> =
        0x03000070 as *mut volatile::VolatilePtr<u32>;

    // ========== I2C 相关常量 ==========

    // I2C状态寄存器位定义
    pub const I2C_STATUS_RXACK: u32 = 0x80; // (1 << 7)
    pub const I2C_STATUS_BUSY: u32 = 0x40; // (1 << 6)
    pub const I2C_STATUS_AL: u32 = 0x20; // (1 << 5)
    pub const I2C_STATUS_TIP: u32 = 0x02; // (1 << 1)
    pub const I2C_STATUS_IF: u32 = 0x01; // (1 << 0)
}

unsafe extern "C" {
    fn start();
}

#[unsafe(no_mangle)]
#[used]
pub static _start: unsafe extern "C" fn() = start;
