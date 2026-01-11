use crate::bindings;

pub struct Timer;

impl Timer {
    pub fn init_tick() {
        unsafe {
            bindings::sys_tick_init();
        }
    }

    pub fn get_tick() -> u32 {
        unsafe { bindings::get_sys_tick() }
    }

    pub fn delay_us(us: u32) {
        unsafe {
            bindings::delay_us(us);
        }
    }

    pub fn delay_ms(ms: u32) {
        unsafe {
            bindings::delay_ms(ms);
        }
    }

    pub fn delay_s(s: u32) {
        unsafe {
            bindings::delay_s(s);
        }
    }
}
