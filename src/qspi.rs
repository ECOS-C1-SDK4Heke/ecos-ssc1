#![allow(static_mut_refs)]

use core::marker::PhantomData;
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::{
    register_bitfields, register_structs,
    registers::{ReadOnly, ReadWrite, WriteOnly},
};

// ========== 寄存器位域定义 ==========
register_bitfields![u32,
    /// STATUS 寄存器
    pub Status [
        BUSY        OFFSET(0)  NUMBITS(1) [],
        DONE        OFFSET(1)  NUMBITS(15) [],
        RESET       OFFSET(16) NUMBITS(1) [],
        INT_FLAG    OFFSET(31) NUMBITS(1) [],
    ],

    /// LEN 寄存器 - C代码写入的是 (比特数 << 16)
    pub Len [
        LENGTH      OFFSET(16) NUMBITS(16) [],
        CTRL        OFFSET(0)  NUMBITS(16) [],
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

// ========== QSPI基地址 ==========
const QSPI0_BASE: usize = crate::bindings::REG_QSPI_0_BASE as usize;

/// 获取QSPI0寄存器实例
#[inline(always)]
fn qspi0() -> &'static mut QspiRegisters {
    unsafe { &mut *(QSPI0_BASE as *mut QspiRegisters) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QspiError {
    Timeout,
    InvalidParameter,
    TransferFailed,
}

// ========== QSPI配置 ==========
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct qspi_config_t {
    pub clkdiv: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct QspiConfig {
    pub clkdiv: u32,
}

impl From<crate::bindings::qspi_config_t> for QspiConfig {
    fn from(c_config: crate::bindings::qspi_config_t) -> Self {
        Self {
            clkdiv: c_config.clkdiv,
        }
    }
}

impl Default for QspiConfig {
    fn default() -> Self {
        Self { clkdiv: 0 }
    }
}

// ========== 主QSPI驱动结构 ==========
pub struct Qspi {
    regs: &'static mut QspiRegisters,
    _private: PhantomData<*mut ()>,
}

impl Qspi {
    /// 创建新的QSPI实例
    pub fn new() -> Self {
        let regs = qspi0();
        Self {
            regs,
            _private: PhantomData,
        }
    }

    /// 初始化QSPI控制器 - 完全按照C代码逻辑
    pub fn init(&mut self, clkdiv: u32) {
        // 完全按照C代码的顺序和值
        self.regs.status.set(0b10000); // STATUS = 0b10000
        self.regs.status.set(0); // STATUS = 0
        self.regs.intcfg.set(0); // INTCFG = 0
        self.regs.dum.set(0); // DUM = 0
        self.regs.clkdiv.set(clkdiv); // CLKDIV = clkdiv
    }

    /// 等待传输完成 - 严格按照C代码逻辑
    pub fn wait_transfer_complete(&self) -> Result<(), QspiError> {
        let mut timeout = 100_000;

        // C代码: while ((REG_QSPI_0_STATUS & 0xFFFF) != 1)
        while timeout > 0 {
            if (self.regs.status.get() & 0xFFFF) == 1 {
                return Ok(());
            }
            timeout -= 1;
        }

        Err(QspiError::Timeout)
    }

    /// 等待传输完成 - 对于write_16/32等函数
    pub fn wait_transfer_complete_full(&self) -> Result<(), QspiError> {
        let mut timeout = 100_000;

        // C代码: while ((REG_QSPI_0_STATUS & 0xFFFFFFFF) != 1)
        while timeout > 0 {
            if (self.regs.status.get() & 0xFFFFFFFF) == 1 {
                return Ok(());
            }
            timeout -= 1;
        }

        Err(QspiError::Timeout)
    }

    // ========== 写入函数 - 完全按照C代码逻辑 ==========

    /// 写入8位数据 - 与C代码完全一致
    pub fn write_u8(&mut self, data: u8) -> Result<(), QspiError> {
        let wdat = (data as u32) << 24; // C代码: ((uint32_t)data) << 24
        self.regs.len.set(0x80000); // C代码: REG_QSPI_0_LEN = 0x80000
        self.regs.txfifo.set(wdat); // C代码: REG_QSPI_0_TXFIFO = wdat
        self.regs.status.set(258); // C代码: REG_QSPI_0_STATUS = 258
        self.wait_transfer_complete() // C代码的while循环
    }

    /// 写入16位数据 - 与C代码完全一致
    pub fn write_u16(&mut self, data: u16) -> Result<(), QspiError> {
        let wdat = (data as u32) << 16; // C代码: ((uint32_t)data) << 16
        self.regs.len.set(0x100000); // C代码: REG_QSPI_0_LEN = 0x100000
        self.regs.txfifo.set(wdat);
        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 写入32位数据 - 与C代码完全一致
    pub fn write_u32(&mut self, data: u32) -> Result<(), QspiError> {
        self.regs.len.set(0x200000); // C代码: REG_QSPI_0_LEN = 0x200000
        self.regs.txfifo.set(data);
        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 写入2个32位数据 - 与C代码完全一致
    pub fn write_u32x2(&mut self, data1: u32, data2: u32) -> Result<(), QspiError> {
        self.regs.len.set(0x400000); // C代码: REG_QSPI_0_LEN = 0x400000
        self.regs.txfifo.set(data1);
        self.regs.txfifo.set(data2);
        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 写入8个32位数据 - 与C代码完全一致
    pub fn write_u32x8(&mut self, data: [u32; 8]) -> Result<(), QspiError> {
        self.regs.len.set(0x1000000); // C代码: REG_QSPI_0_LEN = 0x1000000
        for &d in &data {
            self.regs.txfifo.set(d);
        }
        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 写入16个32位数据 - 与C代码完全一致
    pub fn write_u32x16(&mut self, data: [u32; 16]) -> Result<(), QspiError> {
        self.regs.len.set(0x2000000); // C代码: REG_QSPI_0_LEN = 0x2000000
        for &d in &data {
            self.regs.txfifo.set(d);
        }
        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 写入32个32位数据 - 与C代码完全一致
    pub fn write_u32x32(&mut self, data: [u32; 32]) -> Result<(), QspiError> {
        self.regs.len.set(0x4000000); // C代码: REG_QSPI_0_LEN = 0x4000000
        for &d in &data {
            self.regs.txfifo.set(d);
        }
        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 通用的字节写入函数
    pub fn write_bytes(&mut self, data: &[u8]) -> Result<(), QspiError> {
        if data.is_empty() {
            return Ok(());
        }

        // 按照C代码逻辑：比特数 = 字节数 × 8
        let bits = data.len() * 8;
        let len_value = (bits as u32) << 16;

        self.regs.len.set(len_value);

        // 按C代码的逻辑组织数据
        let mut i = 0;
        while i < data.len() {
            let mut word: u32 = 0;
            for j in 0..4 {
                if i + j < data.len() {
                    let shift = 24 - (j * 8);
                    word |= (data[i + j] as u32) << shift;
                }
            }
            self.regs.txfifo.set(word);
            i += 4;
        }

        self.regs.status.set(258);
        self.wait_transfer_complete_full()
    }

    /// 按照u32写入数据直到全部完成
    pub fn write_words(&mut self, data: &[u32]) -> Result<(), QspiError> {
        if data.is_empty() {
            return Ok(());
        }

        let mut index = 0;
        let total = data.len();

        loop {
            let remaining = total - index;
            if remaining == 0 {
                break;
            }

            match remaining {
                r if r >= 32 => {
                    let chunk: [u32; 32] = data[index..index + 32]
                        .try_into()
                        .map_err(|_| QspiError::InvalidParameter)?;
                    self.write_u32x32(chunk)?;
                    index += 32;
                }
                r if r >= 16 => {
                    let chunk: [u32; 16] = data[index..index + 16]
                        .try_into()
                        .map_err(|_| QspiError::InvalidParameter)?;
                    self.write_u32x16(chunk)?;
                    index += 16;
                }
                r if r >= 8 => {
                    let chunk: [u32; 8] = data[index..index + 8]
                        .try_into()
                        .map_err(|_| QspiError::InvalidParameter)?;
                    self.write_u32x8(chunk)?;
                    index += 8;
                }
                r if r >= 2 => {
                    self.write_u32x2(data[index], data[index + 1])?;
                    index += 2;
                }
                1 => {
                    self.write_u32(data[index])?;
                    index += 1;
                }
                _ => unreachable!(),
            }
        }

        Ok(())
    }

    /// 从接收FIFO读取32位数据
    pub fn read_u32(&self) -> u32 {
        self.regs.rxfifo.get()
    }

    /// 设置传输地址
    pub fn set_address(&mut self, address: u32) {
        self.regs.adr.set(address);
    }

    /// 设置时钟分频
    pub fn set_clock_divider(&mut self, clkdiv: u32) {
        self.regs.clkdiv.set(clkdiv);
    }
}

// ========== 简单的全局实例（无原子操作） ==========

static mut QSPI_INSTANCE: Option<Qspi> = None;

/// 获取QSPI实例 - 简单全局访问
pub fn get_qspi() -> Option<&'static mut Qspi> {
    unsafe { QSPI_INSTANCE.as_mut() }
}

/// 初始化QSPI
pub fn init_qspi(clkdiv: u32) {
    unsafe {
        if QSPI_INSTANCE.is_none() {
            let mut qspi = Qspi::new();
            qspi.init(clkdiv);
            QSPI_INSTANCE = Some(qspi);
        }
    }
}

/// 便捷函数：写入8位数据
pub fn write_u8(data: u8) -> Result<(), QspiError> {
    unsafe {
        if let Some(qspi) = QSPI_INSTANCE.as_mut() {
            qspi.write_u8(data)
        } else {
            Err(QspiError::TransferFailed)
        }
    }
}

/// 便捷函数：写入16位数据
pub fn write_u16(data: u16) -> Result<(), QspiError> {
    unsafe {
        if let Some(qspi) = QSPI_INSTANCE.as_mut() {
            qspi.write_u16(data)
        } else {
            Err(QspiError::TransferFailed)
        }
    }
}

/// 便捷函数：写入32位数据
pub fn write_u32(data: u32) -> Result<(), QspiError> {
    unsafe {
        if let Some(qspi) = QSPI_INSTANCE.as_mut() {
            qspi.write_u32(data)
        } else {
            Err(QspiError::TransferFailed)
        }
    }
}

/// 全局函数：按照u8写入数据直到全部完成
pub fn write_bytes(data: &[u8]) -> Result<(), QspiError> {
    unsafe {
        if let Some(qspi) = QSPI_INSTANCE.as_mut() {
            qspi.write_bytes(data)
        } else {
            Err(QspiError::TransferFailed)
        }
    }
}

/// 全局函数：按照u32写入数据直到全部完成
pub fn write_words(data: &[u32]) -> Result<(), QspiError> {
    unsafe {
        if let Some(qspi) = QSPI_INSTANCE.as_mut() {
            qspi.write_words(data)
        } else {
            Err(QspiError::TransferFailed)
        }
    }
}
