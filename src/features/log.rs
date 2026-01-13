//! # LOG
//!
//! > todo: 之后找到特权级原因后，把log="0.4"接回来...
//!
//! ## 特性
//! - `log`: 启用日志系统
//! - `log-colored`: 启用彩色ANSI输出（手动实现）
//!
//! ## 使用示例
//! ```
//! use ecos_ssc1::features::log::init_logger;
//!
//! init_logger();
//!
//! use ecos_ssc1::log::info;
//! info!("系统已启动");
//! ```

#[allow(unused)]
use crate::{print, println};
use core::fmt;

// ========== 日志级别定义 ==========

/// 日志级别枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// 最高级别：严重的错误
    Error = 0,
    /// 警告级别
    Warn = 1,
    /// 信息级别
    Info = 2,
    /// 调试级别
    Debug = 3,
    /// 最低级别：详细的跟踪信息
    Trace = 4,
}

impl Level {
    /// 获取级别的字符串表示
    pub fn as_str(&self) -> &'static str {
        match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN ",
            Level::Info => "INFO ",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        }
    }

    /// 获取级别的简短表示
    pub fn as_short(&self) -> char {
        match self {
            Level::Error => 'E',
            Level::Warn => 'W',
            Level::Info => 'I',
            Level::Debug => 'D',
            Level::Trace => 'T',
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 日志级别过滤器
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LevelFilter {
    /// 关闭所有日志
    Off,
    /// 只显示错误
    Error,
    /// 显示错误和警告
    Warn,
    /// 显示错误、警告和信息
    Info,
    /// 显示错误、警告、信息和调试
    Debug,
    /// 显示所有日志
    Trace,
}

impl LevelFilter {
    /// 检查给定的级别是否满足过滤器要求
    pub fn accepts(&self, level: Level) -> bool {
        match self {
            LevelFilter::Off => false,
            LevelFilter::Error => level as usize <= Level::Error as usize,
            LevelFilter::Warn => level as usize <= Level::Warn as usize,
            LevelFilter::Info => level as usize <= Level::Info as usize,
            LevelFilter::Debug => level as usize <= Level::Debug as usize,
            LevelFilter::Trace => level as usize <= Level::Trace as usize,
        }
    }
}

impl Default for LevelFilter {
    fn default() -> Self {
        LevelFilter::Info
    }
}

impl fmt::Display for LevelFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LevelFilter::Off => write!(f, "Off"),
            LevelFilter::Error => write!(f, "Error"),
            LevelFilter::Warn => write!(f, "Warn"),
            LevelFilter::Info => write!(f, "Info"),
            LevelFilter::Debug => write!(f, "Debug"),
            LevelFilter::Trace => write!(f, "Trace"),
        }
    }
}

// ========== ANSI 颜色定义 ==========

#[cfg(feature = "log-colored")]
#[allow(unused)]
mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";

    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";

    pub const BRIGHT_RED: &str = "\x1b[91m";
    pub const BRIGHT_GREEN: &str = "\x1b[92m";
    pub const BRIGHT_YELLOW: &str = "\x1b[93m";
    pub const BRIGHT_BLUE: &str = "\x1b[94m";
    pub const BRIGHT_MAGENTA: &str = "\x1b[95m";
    pub const BRIGHT_CYAN: &str = "\x1b[96m";

    /// 根据日志级别获取颜色
    pub fn color_for_level(level: crate::features::log::Level) -> &'static str {
        match level {
            crate::features::log::Level::Error => BRIGHT_RED,
            crate::features::log::Level::Warn => BRIGHT_YELLOW,
            crate::features::log::Level::Info => BRIGHT_GREEN,
            crate::features::log::Level::Debug => BRIGHT_CYAN,
            crate::features::log::Level::Trace => BRIGHT_MAGENTA,
        }
    }
}

// ========== 元数据定义 ==========

/// 日志元数据
pub struct Metadata<'a> {
    level: Level,
    target: &'a str,
}

impl<'a> Metadata<'a> {
    /// 创建新的元数据
    pub const fn new(level: Level, target: &'a str) -> Self {
        Self { level, target }
    }

    /// 获取日志级别
    pub fn level(&self) -> Level {
        self.level
    }

    /// 获取目标模块/组件
    pub fn target(&self) -> &'a str {
        self.target
    }
}

// ========== 记录定义 ==========

/// 日志记录
pub struct Record<'a> {
    metadata: Metadata<'a>,
    args: fmt::Arguments<'a>,
}

impl<'a> Record<'a> {
    /// 创建新的日志记录
    pub fn new(metadata: Metadata<'a>, args: fmt::Arguments<'a>) -> Self {
        Self { metadata, args }
    }

    /// 获取元数据
    pub fn metadata(&self) -> &Metadata<'a> {
        &self.metadata
    }

    /// 获取格式化参数
    pub fn args(&self) -> fmt::Arguments<'a> {
        self.args
    }

    /// 获取日志级别
    pub fn level(&self) -> Level {
        self.metadata.level
    }

    /// 获取目标模块
    pub fn target(&self) -> &'a str {
        self.metadata.target
    }
}

// ========== 日志记录器实现 ==========

/// 日志记录器结构体
pub struct EcosLogger {
    /// 是否启用彩色输出
    use_colors: bool,
    /// 日志级别过滤器
    max_level: LevelFilter,
    /// 是否显示时间戳
    show_timestamp: bool,
    /// 是否已初始化
    initialized: bool,
}

impl EcosLogger {
    /// 创建新的日志记录器（未初始化状态）
    const fn new() -> Self {
        Self {
            use_colors: cfg!(feature = "log-colored"),
            max_level: LevelFilter::Info,
            show_timestamp: false,
            initialized: false,
        }
    }

    /// 初始化日志记录器
    pub fn init(&mut self, use_colors: bool, max_level: LevelFilter, show_timestamp: bool) {
        self.use_colors = use_colors;
        self.max_level = max_level;
        self.show_timestamp = show_timestamp;
        self.initialized = true;
    }

    /// 检查是否已初始化
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// 检查是否接受指定级别的日志
    fn accepts(&self, level: Level) -> bool {
        self.initialized && self.max_level.accepts(level)
    }

    /// 获取当前时间戳
    fn get_timestamp(&self) -> Option<u32> {
        // 只有在启用了时间戳且可以安全调用时才获取时间戳
        if self.show_timestamp {
            // 使用不安全方式获取时间戳
            Some(unsafe { crate::bindings::get_sys_tick() })
        } else {
            None
        }
    }

    /// 格式化并输出日志记录
    fn log_record(&self, record: &Record) {
        // 检查是否接受该级别的日志
        if !self.accepts(record.level()) {
            return;
        }

        // 输出时间戳（如果需要）
        if let Some(timestamp) = self.get_timestamp() {
            print!("[{:08X}] ", timestamp);
        }

        // 输出日志级别和目标
        #[cfg(feature = "log-colored")]
        if self.use_colors {
            use ansi::{BOLD, BRIGHT_BLUE, RESET, color_for_level};

            let color = color_for_level(record.level());
            let text = record.level().as_str();

            print!("[{}", color);
            print!("{}", BOLD);
            print!("{}", text);
            print!("{}", RESET);
            print!("] [");
            print!("{}", BRIGHT_BLUE);
            print!("{}", record.target());
            print!("{}", RESET);
            print!("] ");

            // 输出消息内容
            println!("{}", record.args());
        } else {
            // 无彩色版本
            print!("[{}] [{}] ", record.level(), record.target());
            println!("{}", record.args());
        }

        #[cfg(not(feature = "log-colored"))]
        {
            // 无彩色版本
            print!("[{}] [{}] ", record.level(), record.target());
            println!("{}", record.args());
        }
    }
}

// 全局日志记录器实例
static mut LOGGER: EcosLogger = EcosLogger::new();

// ========== 初始化函数 ==========

/// 初始化日志系统
///
/// # 参数
/// - `use_colors`: 是否启用彩色输出（仅当log-colored特性启用时有效）
/// - `max_level`: 最大日志级别
/// - `show_timestamp`: 是否显示时间戳
///
/// # 示例
/// ```
/// // 使用默认配置初始化
/// init_logger();
///
/// // 自定义配置
/// init_with_config(false, LevelFilter::Debug, true);
/// ```
pub fn init_with_config(use_colors: bool, max_level: LevelFilter, show_timestamp: bool) {
    // Rust 2024 中允许使用 addr_of_mut! 来获取静态可变变量的地址
    use core::ptr::addr_of_mut;

    unsafe {
        // 使用 addr_of_mut! 而不是直接引用
        let logger = addr_of_mut!(LOGGER);
        (*logger).init(use_colors, max_level, show_timestamp);
    }
}

/// 使用默认配置初始化日志系统
///
/// 默认配置：
/// - 彩色输出：如果启用了log-colored特性则启用
/// - 日志级别：Info
/// - 时间戳：不显示
pub fn init_logger() {
    init_with_config(cfg!(feature = "log-colored"), LevelFilter::Info, false);
}

/// 检查日志系统是否已初始化
pub fn is_initialized() -> bool {
    // Rust 2024 中允许使用 addr_of! 来获取静态变量的地址
    use core::ptr::addr_of;

    unsafe {
        let logger = addr_of!(LOGGER);
        (*logger).is_initialized()
    }
}

/// 获取当前日志级别过滤器
pub fn max_level() -> LevelFilter {
    use core::ptr::addr_of;

    unsafe {
        let logger = addr_of!(LOGGER);
        (*logger).max_level
    }
}

/// 设置日志级别过滤器
pub fn set_max_level(level: LevelFilter) {
    use core::ptr::addr_of_mut;

    unsafe {
        let logger = addr_of_mut!(LOGGER);
        if (*logger).is_initialized() {
            (*logger).max_level = level;
        }
    }
}

/// 内部日志函数（供宏使用）
#[doc(hidden)]
pub fn __log_internal(level: Level, target: &'static str, args: fmt::Arguments) {
    use core::ptr::addr_of;

    unsafe {
        let logger = addr_of!(LOGGER);
        if (*logger).is_initialized() && (*logger).accepts(level) {
            let metadata = Metadata::new(level, target);
            let record = Record::new(metadata, args);
            (*logger).log_record(&record);
        }
    }
}

// ========== 日志宏 ==========

/// 通用的日志宏
#[macro_export]
macro_rules! __log {
    ($level:expr, $($arg:tt)*) => {{
        if $crate::features::log::is_initialized() {
            // 使用模块路径作为目标
            let target = module_path!();
            $crate::features::log::__log_internal($level, target, format_args!($($arg)*));
        }
    }};
}

/// 错误级别日志宏
#[macro_export]
macro_rules! __error {
    ($($arg:tt)*) => {{
        $crate::log!($crate::features::log::Level::Error, $($arg)*);
    }};
}

/// 警告级别日志宏
#[macro_export]
macro_rules! __warn {
    ($($arg:tt)*) => {{
        $crate::log!($crate::features::log::Level::Warn, $($arg)*);
    }};
}

/// 信息级别日志宏
#[macro_export]
macro_rules! __info {
    ($($arg:tt)*) => {{
        $crate::log!($crate::features::log::Level::Info, $($arg)*);
    }};
}

/// 调试级别日志宏
#[macro_export]
macro_rules! __debug {
    ($($arg:tt)*) => {{
        $crate::log!($crate::features::log::Level::Debug, $($arg)*);
    }};
}

/// 跟踪级别日志宏
#[macro_export]
macro_rules! __trace {
    ($($arg:tt)*) => {{
        $crate::log!($crate::features::log::Level::Trace, $($arg)*);
    }};
}

/// 便利的日志初始化宏
#[macro_export]
macro_rules! init_logging {
    () => {{
        $crate::features::log::init_logger();
        $crate::log::info!("日志系统已初始化");
    }};

    ($level:expr) => {{
        use $crate::features::log::{LevelFilter, init_with_config};
        let level = $level;
        init_with_config(cfg!(feature = "log-colored"), level, false);
        $crate::log::info!("日志系统已初始化，级别: {}", level);
    }};

    ($level:expr, $colors:expr) => {{
        use $crate::features::log::{LevelFilter, init_with_config};
        let level = $level;
        let use_colors = $colors;
        init_with_config(use_colors, level, false);
        $crate::log::info!(
            "日志系统已初始化，级别: {}，彩色输出: {}",
            level,
            if use_colors { "启用" } else { "禁用" }
        );
    }};

    ($level:expr, $colors:expr, $timestamp:expr) => {{
        use $crate::features::log::{LevelFilter, init_with_config};
        let level = $level;
        let use_colors = $colors;
        let show_timestamp = $timestamp;
        init_with_config(use_colors, level, show_timestamp);
        $crate::log::info!(
            "日志系统已初始化，级别: {}，彩色输出: {}，时间戳: {}",
            level,
            if use_colors { "启用" } else { "禁用" },
            if show_timestamp { "启用" } else { "禁用" }
        );
    }};
}

// 保持不变
pub use crate::{
    __debug as debug, __error as error, __info as info, __log as log, __trace as trace,
    __warn as warn,
};
