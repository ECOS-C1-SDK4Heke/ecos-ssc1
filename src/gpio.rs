use crate::bindings;

/// 原始的奇怪排列，按宏定义的顺序来的
pub struct Gpio;

impl Gpio {
    pub fn set_level(pin: u32, level: bool) {
        let level_val = if level {
            bindings::gpio_level_t_GPIO_LEVEL_HIGH
        } else {
            bindings::gpio_level_t_GPIO_LEVEL_LOW
        };
        unsafe {
            bindings::gpio_set_level(pin, level_val);
        }
    }

    pub fn get_level(pin: u32) -> bool {
        unsafe { bindings::gpio_get_level(pin) != 0 }
    }

    pub fn set_function(pin: u32, func: u32) {
        unsafe {
            bindings::gpio_set_function(pin, func);
        }
    }

    pub fn config(pins: u64, mode: u32) {
        let config = bindings::gpio_config_t {
            pin_bit_mask: pins,
            mode,
        };
        unsafe {
            bindings::gpio_config(&config);
        }
    }
}

/// 按照排针号排列的，1-16
pub struct GpioPin;

const PIN_TO_GPIO: [u32; 16] = [9, 5, 8, 0, 7, 1, 6, 10, 11, 12, 13, 14, 15, 2, 3, 4];

const GPIO_TO_PIN: [u32; 16] = [4, 14, 15, 16, 2, 6, 5, 3, 1, 8, 9, 10, 11, 12, 13, 7];

impl GpioPin {
    pub fn pin_to_gpio(pin: u32) -> Option<u32> {
        if pin >= 1 && pin <= 16 {
            Some(PIN_TO_GPIO[(pin - 1) as usize])
        } else {
            None
        }
    }

    pub fn gpio_to_pin(gpio: u32) -> Option<u32> {
        if gpio <= 15 {
            Some(GPIO_TO_PIN[gpio as usize])
        } else {
            None
        }
    }

    pub fn set_level(pin: u32, level: bool) -> Option<()> {
        let gpio = Self::pin_to_gpio(pin)?;
        Gpio::set_level(gpio, level);
        Some(())
    }

    pub fn get_level(pin: u32) -> Option<bool> {
        let gpio = Self::pin_to_gpio(pin)?;
        Some(Gpio::get_level(gpio))
    }

    pub fn set_function(pin: u32, func: u32) -> Option<()> {
        let gpio = Self::pin_to_gpio(pin)?;
        Gpio::set_function(gpio, func);
        Some(())
    }

    pub fn config_pins(pin_mask: u16, mode: u32) {
        let mut gpio_mask: u64 = 0;

        for pin in 1..=16 {
            if (pin_mask >> (pin - 1)) & 1 != 0 {
                if let Some(gpio) = Self::pin_to_gpio(pin) {
                    gpio_mask |= 1 << gpio;
                }
            }
        }

        Gpio::config(gpio_mask, mode);
    }

    pub fn get_all_pins() -> u16 {
        let mut result = 0u16;

        for pin in 1..=16 {
            if let Some(level) = Self::get_level(pin) {
                if level {
                    result |= 1 << (pin - 1);
                }
            }
        }

        result
    }
}
