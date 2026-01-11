// src/qspi.rs
#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_variables)]

use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicBool, Ordering};

use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};
use tock_registers::{
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

// ========== 导入C函数 ==========
use crate::bindings::delay_us;

// ========== 寄存器位域定义 ==========
register_bitfields![u32,
    /// STATUS 寄存器
    pub Status [
        BUSY        OFFSET(0)  NUMBITS(1) [],
        DONE        OFFSET(1)  NUMBITS(15) [],
        RESET       OFFSET(16) NUMBITS(1) [],
        INT_FLAG    OFFSET(31) NUMBITS(1) [],
    ],

    /// LEN 寄存器
    pub Len [
        LENGTH      OFFSET(20) NUMBITS(12) [],
        CTRL        OFFSET(0)  NUMBITS(20) [],
    ],

    /// CMD 寄存器
    pub Cmd [
        MODE        OFFSET(0)  NUMBITS(2) [
            Standard = 0,
            Dual = 1,
            Quad = 2,
        ],
        TYPE        OFFSET(2)  NUMBITS(2) [
            WriteOnly = 0,
            ReadOnly = 1,
            WriteRead = 2,
        ],
        ADDR_BYTES  OFFSET(4)  NUMBITS(2) [
            Addr0Byte = 0,
            Addr1Byte = 1,
            Addr2Byte = 2,
            Addr3Byte = 3,
        ],
        DATA_BYTES  OFFSET(6)  NUMBITS(2) [
            Data0Byte = 0,
            Data1Byte = 1,
            Data2Byte = 2,
            Data4Byte = 3,
        ],
        DMA_EN      OFFSET(8)  NUMBITS(1) [],
        START       OFFSET(31) NUMBITS(1) [],
    ],

    /// INTCFG 寄存器
    pub IntCfg [
        TX_COMPLETE OFFSET(0)  NUMBITS(1) [],
        RX_COMPLETE OFFSET(1)  NUMBITS(1) [],
        TX_THRESH   OFFSET(2)  NUMBITS(1) [],
        RX_THRESH   OFFSET(3)  NUMBITS(1) [],
        TX_THRESH_VAL OFFSET(8)  NUMBITS(4) [],
        RX_THRESH_VAL OFFSET(12) NUMBITS(4) [],
    ],

    /// INTSTA 寄存器
    pub IntSta [
        TX_COMPLETE OFFSET(0)  NUMBITS(1) [],
        RX_COMPLETE OFFSET(1)  NUMBITS(1) [],
        TX_THRESH   OFFSET(2)  NUMBITS(1) [],
        RX_THRESH   OFFSET(3)  NUMBITS(1) [],
    ],
];

// ========== 寄存器结构体定义 ==========
register_structs! {
    #[allow(non_snake_case)]
    pub QspiRegisters {
        (0x00 => status: ReadWrite<u32, Status::Register>),
        (0x04 => clkdiv: ReadWrite<u32>),
        (0x08 => cmd: ReadWrite<u32, Cmd::Register>),
        (0x0C => adr: ReadWrite<u32>),
        (0x10 => len: ReadWrite<u32, Len::Register>),
        (0x14 => dum: ReadWrite<u32>),
        (0x18 => txfifo: WriteOnly<u32>),
        (0x1C => _reserved1: [u8; 4]),
        (0x20 => rxfifo: ReadOnly<u32>),
        (0x24 => intcfg: ReadWrite<u32, IntCfg::Register>),
        (0x28 => intsta: ReadWrite<u32, IntSta::Register>),
        (0x2C => @END),
    }
}

// ========== QSPI实例 ==========
const QSPI0_BASE: usize = crate::bindings::REG_QSPI_0_BASE as usize;

/// 获取QSPI0寄存器实例
#[inline(always)]
fn qspi0() -> &'static mut QspiRegisters {
    unsafe { &mut *(QSPI0_BASE as *mut QspiRegisters) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QspiError {
    Timeout,
    FifoOverflow,
    FifoUnderflow,
    InvalidParameter,
    Busy,
    TransferFailed,
    InitFailed,
    AlreadyInitialized,
    NotInitialized,
}

// ========== QSPI配置 ==========
#[derive(Debug, Clone, Copy)]
pub struct QspiConfig {
    pub clkdiv: u32,
    pub dummy_cycles: u8,
    pub interrupt_config: Option<InterruptConfig>,
}

impl Default for QspiConfig {
    fn default() -> Self {
        Self {
            clkdiv: 1,
            dummy_cycles: 0,
            interrupt_config: None,
        }
    }
}

/// 中断配置
#[derive(Debug, Clone, Copy)]
pub struct InterruptConfig {
    pub tx_complete_enable: bool,
    pub rx_complete_enable: bool,
    pub tx_threshold_enable: bool,
    pub rx_threshold_enable: bool,
    pub tx_threshold: u8,
    pub rx_threshold: u8,
}

// ========== QSPI传输模式 ==========
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QspiMode {
    Standard,
    Dual,
    Quad,
}

// ========== 主QSPI驱动结构 ==========
pub struct Qspi {
    regs: &'static mut QspiRegisters,
    config: QspiConfig,
    _private: PhantomData<*mut ()>,
}

impl Qspi {
    /// 创建新的QSPI实例
    pub fn new(config: QspiConfig) -> Self {
        let regs = qspi0();
        Self {
            regs,
            config,
            _private: PhantomData,
        }
    }

    /// 初始化QSPI控制器
    pub fn init(&mut self) {
        self.regs.status.write(Status::RESET::SET);
        unsafe {
            delay_us(10);
        }
        self.regs.status.write(Status::RESET::CLEAR);
        unsafe {
            delay_us(10);
        }

        self.regs.clkdiv.set(self.config.clkdiv);

        self.regs.intcfg.set(0b00000);

        self.regs.dum.set(self.config.dummy_cycles as u32);

        if let Some(int_cfg) = self.config.interrupt_config {
            self.regs.intcfg.write(
                IntCfg::TX_COMPLETE.val(int_cfg.tx_complete_enable as u32)
                    + IntCfg::RX_COMPLETE.val(int_cfg.rx_complete_enable as u32)
                    + IntCfg::TX_THRESH.val(int_cfg.tx_threshold_enable as u32)
                    + IntCfg::RX_THRESH.val(int_cfg.rx_threshold_enable as u32)
                    + IntCfg::TX_THRESH_VAL.val(int_cfg.tx_threshold as u32)
                    + IntCfg::RX_THRESH_VAL.val(int_cfg.rx_threshold as u32),
            );
        }
    }

    /// 检查是否繁忙
    pub fn is_busy(&self) -> bool {
        self.regs.status.is_set(Status::BUSY)
    }

    /// 等待传输完成 - 最大等待100ms
    fn wait_transfer_complete(&self) -> Result<(), QspiError> {
        let mut timeout = 100000;

        while timeout > 0 {
            let status = self.regs.status.get();
            if (status & 0xFFFF) == 1 {
                return Ok(());
            }
            timeout -= 1;
        }

        Err(QspiError::Timeout)
    }

    /// 设置传输长度 - 允许 8bit 16bit 32bit
    fn set_transfer_length(&mut self, bits: u32) {
        let len_value = bits << 20;
        self.regs.len.set(len_value);
    }

    /// 写入8位数据 - 完全按照C代码逻辑
    pub fn write_u8(&mut self, data: u8) -> Result<(), QspiError> {
        self.set_transfer_length(8);

        let wdat = (data as u32) << 24;
        self.regs.txfifo.set(wdat);

        self.regs.status.set(258);

        self.wait_transfer_complete()
    }

    /// 写入16位数据
    pub fn write_u16(&mut self, data: u16) -> Result<(), QspiError> {
        self.set_transfer_length(16);
        let wdat = (data as u32) << 16;
        self.regs.txfifo.set(wdat);
        self.regs.status.set(258);
        self.wait_transfer_complete()
    }

    /// 写入32位数据
    pub fn write_u32(&mut self, data: u32) -> Result<(), QspiError> {
        self.set_transfer_length(32);
        self.regs.txfifo.set(data);
        self.regs.status.set(258);
        self.wait_transfer_complete()
    }

    /// 写入2个32位数据
    pub fn write_u32x2(&mut self, data1: u32, data2: u32) -> Result<(), QspiError> {
        self.set_transfer_length(64);
        self.regs.txfifo.set(data1);
        self.regs.txfifo.set(data2);
        self.regs.status.set(258);
        self.wait_transfer_complete()
    }

    /// 写入8个32位数据
    pub fn write_u32x8(&mut self, data: [u32; 8]) -> Result<(), QspiError> {
        self.set_transfer_length(256);
        for &d in &data {
            self.regs.txfifo.set(d);
        }
        self.regs.status.set(258);
        self.wait_transfer_complete()
    }

    /// 写入16个32位数据
    pub fn write_u32x16(&mut self, data: [u32; 16]) -> Result<(), QspiError> {
        self.set_transfer_length(512);
        for &d in &data {
            self.regs.txfifo.set(d);
        }
        self.regs.status.set(258);
        self.wait_transfer_complete()
    }

    /// 写入32个32位数据
    pub fn write_u32x32(&mut self, data: [u32; 32]) -> Result<(), QspiError> {
        self.set_transfer_length(1024);
        for &d in &data {
            self.regs.txfifo.set(d);
        }
        self.regs.status.set(258);
        self.wait_transfer_complete()
    }

    /// 写入字节数组
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), QspiError> {
        if data.is_empty() {
            return Ok(());
        }

        let mut i = 0;
        while i + 3 < data.len() {
            let word = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
            self.write_u32(word)?;
            i += 4;
        }

        match data.len() - i {
            0 => Ok(()),
            1 => self.write_u8(data[i]),
            2 => {
                let word = u16::from_le_bytes([data[i], data[i + 1]]);
                self.write_u16(word)
            }
            3 => {
                let word = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], 0]);
                self.write_u32(word)
            }
            _ => unreachable!(),
        }
    }

    /// 从接收FIFO读取32位数据
    pub fn read_u32(&self) -> Result<u32, QspiError> {
        Ok(self.regs.rxfifo.get())
    }

    /// 批量读取数据
    pub fn read_bytes(&mut self, data: &mut [u8]) -> Result<(), QspiError> {
        if data.is_empty() {
            return Ok(());
        }

        let mut i = 0;
        while i + 3 < data.len() {
            let word = self.read_u32()?;
            let bytes = word.to_le_bytes();
            data[i] = bytes[0];
            data[i + 1] = bytes[1];
            data[i + 2] = bytes[2];
            data[i + 3] = bytes[3];
            i += 4;
        }

        if i < data.len() {
            let word = self.read_u32()?;
            let bytes = word.to_le_bytes();
            let remaining = data.len() - i;
            for j in 0..remaining {
                data[i + j] = bytes[j];
            }
        }

        Ok(())
    }

    /// 设置传输地址
    pub fn set_address(&mut self, address: u32) {
        self.regs.adr.set(address);
    }

    /// 设置虚拟周期
    pub fn set_dummy_cycles(&mut self, cycles: u8) {
        self.regs.dum.set(cycles as u32);
    }

    /// 设置时钟分频
    pub fn set_clock_divider(&mut self, clkdiv: u32) {
        self.regs.clkdiv.set(clkdiv);
    }

    /// 执行自定义命令传输
    pub fn execute_command(
        &mut self,
        command: u8,
        address: Option<u32>,
        dummy_cycles: u8,
        tx_data: &[u8],
        rx_data: &mut [u8],
    ) -> Result<(), QspiError> {
        // 设置地址
        if let Some(addr) = address {
            self.set_address(addr);
        }

        // 设置虚拟周期
        self.set_dummy_cycles(dummy_cycles);

        // 发送命令
        self.write_u8(command)?;

        // 发送数据
        if !tx_data.is_empty() {
            self.write_bytes(tx_data)?;
        }

        // 接收数据
        if !rx_data.is_empty() {
            // 为读取操作发送虚拟数据 - 假设最大读取长度为256字节
            const MAX_READ_SIZE: usize = 256;
            let dummy_tx = [0u8; MAX_READ_SIZE];
            let read_len = rx_data.len();

            // 只发送实际需要读取的长度
            self.write_bytes(&dummy_tx[..read_len])?;

            // 等待数据就绪
            self.wait_transfer_complete()?;

            // 读取数据
            self.read_bytes(rx_data)?;
        }

        Ok(())
    }
}

// ========== 高级接口：Flash操作 ==========
pub struct QspiFlash {
    qspi: Qspi,
}

impl QspiFlash {
    /// 创建QSPI Flash接口
    pub fn new(config: QspiConfig) -> Self {
        let mut qspi = Qspi::new(config);
        qspi.init();
        Self { qspi }
    }

    /// 读取设备ID（命令0x9F）
    pub fn read_id(&mut self) -> Result<[u8; 3], QspiError> {
        let mut id = [0u8; 3];
        self.qspi.execute_command(0x9F, None, 0, &[], &mut id)?;
        Ok(id)
    }

    /// 读取状态寄存器（命令0x05）
    pub fn read_status(&mut self) -> Result<u8, QspiError> {
        let mut status = [0u8; 1];
        self.qspi.execute_command(0x05, None, 0, &[], &mut status)?;
        Ok(status[0])
    }

    /// 等待Flash空闲
    pub fn wait_idle(&mut self, timeout_ms: u32) -> Result<(), QspiError> {
        let mut retry = timeout_ms * 100; // 每10us检查一次

        while retry > 0 {
            let status = self.read_status()?;
            if (status & 0x01) == 0 {
                // 检查WIP位
                return Ok(());
            }
            retry -= 1;
            unsafe {
                delay_us(10);
            }
        }

        Err(QspiError::Timeout)
    }

    /// 读取数据（命令0x03）
    pub fn read(&mut self, address: u32, data: &mut [u8]) -> Result<(), QspiError> {
        if data.is_empty() {
            return Ok(());
        }

        self.wait_idle(100)?;
        self.qspi.execute_command(0x03, Some(address), 0, &[], data)
    }

    /// 使能写操作（命令0x06）
    pub fn write_enable(&mut self) -> Result<(), QspiError> {
        self.qspi.execute_command(0x06, None, 0, &[], &mut [])?;
        unsafe {
            delay_us(10);
        }
        Ok(())
    }

    /// 禁止写操作（命令0x04）
    pub fn write_disable(&mut self) -> Result<(), QspiError> {
        self.qspi.execute_command(0x04, None, 0, &[], &mut [])?;
        unsafe {
            delay_us(10);
        }
        Ok(())
    }

    /// 擦除扇区（命令0x20，4KB）
    pub fn erase_sector(&mut self, sector_address: u32) -> Result<(), QspiError> {
        self.write_enable()?;
        self.qspi
            .execute_command(0x20, Some(sector_address), 0, &[], &mut [])?;
        self.wait_idle(5000)?; // 扇区擦除最多5秒
        unsafe {
            delay_us(100);
        }
        Ok(())
    }

    /// 擦除整个芯片（命令0xC7）
    pub fn chip_erase(&mut self) -> Result<(), QspiError> {
        self.write_enable()?;
        self.qspi.execute_command(0xC7, None, 0, &[], &mut [])?;
        self.wait_idle(30000)?; // 芯片擦除最多30秒
        Ok(())
    }

    /// 页编程（命令0x02）
    pub fn page_program(&mut self, address: u32, data: &[u8]) -> Result<(), QspiError> {
        if data.is_empty() || data.len() > 256 {
            return Err(QspiError::InvalidParameter);
        }

        self.write_enable()?;
        self.qspi
            .execute_command(0x02, Some(address), 0, data, &mut [])?;
        self.wait_idle(10)?; // 页编程最多10ms
        Ok(())
    }

    /// 写状态寄存器（命令0x01）
    pub fn write_status(&mut self, status: u8) -> Result<(), QspiError> {
        self.write_enable()?;
        let data = [status];
        self.qspi.execute_command(0x01, None, 0, &data, &mut [])?;
        self.wait_idle(10)?;
        Ok(())
    }
}

// ========== 全局QSPI实例 ==========
static QSPI_INITIALIZED: AtomicBool = AtomicBool::new(false);

struct QspiInstance {
    instance: UnsafeCell<Option<Qspi>>,
}

unsafe impl Sync for QspiInstance {}

static QSPI_INSTANCE: QspiInstance = QspiInstance {
    instance: UnsafeCell::new(None),
};

/// 获取全局QSPI单例
pub fn global_qspi(config: QspiConfig) -> Result<&'static mut Qspi, QspiError> {
    unsafe {
        let instance_ptr = QSPI_INSTANCE.instance.get();
        let instance = &mut *instance_ptr;

        if let Some(qspi) = instance {
            Ok(qspi)
        } else {
            if QSPI_INITIALIZED.load(Ordering::Relaxed) {
                return Err(QspiError::AlreadyInitialized);
            }

            let mut qspi = Qspi::new(config);
            qspi.init();
            *instance = Some(qspi);
            QSPI_INITIALIZED.store(true, Ordering::Relaxed);

            Ok(instance.as_mut().unwrap())
        }
    }
}

/// 获取已初始化的QSPI实例
pub fn get_qspi() -> Result<&'static mut Qspi, QspiError> {
    unsafe {
        let instance_ptr = QSPI_INSTANCE.instance.get();
        let instance = &mut *instance_ptr;

        if let Some(qspi) = instance {
            Ok(qspi)
        } else {
            Err(QspiError::NotInitialized)
        }
    }
}

/// 初始化QSPI - 使用前必须初始化！
pub fn init_qspi(config: QspiConfig) -> Result<(), QspiError> {
    unsafe {
        if QSPI_INITIALIZED.load(Ordering::Relaxed) {
            return Err(QspiError::AlreadyInitialized);
        }

        let instance_ptr = QSPI_INSTANCE.instance.get();
        let instance = &mut *instance_ptr;

        let mut qspi = Qspi::new(config);
        qspi.init();
        *instance = Some(qspi);
        QSPI_INITIALIZED.store(true, Ordering::Relaxed);

        Ok(())
    }
}

/// 释放QSPI资源
pub fn deinit_qspi() -> Result<(), QspiError> {
    unsafe {
        let instance_ptr = QSPI_INSTANCE.instance.get();
        let instance = &mut *instance_ptr;

        *instance = None;
        QSPI_INITIALIZED.store(false, Ordering::Relaxed);
    }

    Ok(())
}

// ========== 简单的互斥包装 ==========
pub struct QspiMutex {
    qspi: Qspi,
}

impl QspiMutex {
    pub fn new(config: QspiConfig) -> Self {
        let qspi = Qspi::new(config);
        Self { qspi }
    }

    pub fn lock<R>(&mut self, f: impl FnOnce(&mut Qspi) -> R) -> R {
        f(&mut self.qspi)
    }
}
