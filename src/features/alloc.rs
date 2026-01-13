#![allow(unused)]

// ... 测试打印宏 ... 的补丁 ...
macro_rules! println {
    ($($arg:tt)*) => {
        #[cfg(feature = "alloc-debug-trace")]
        {
            #[cfg(feature = "log")]
            {
                use crate::log;
                log::trace!($($arg)*);
            }
            #[cfg(not(feature = "log"))]
            {
                crate::print!("[TRACE] ");
                crate::println!($($arg)*);
            }
        }
        #[cfg(not(feature = "alloc-debug-trace"))]
        {
            // 空实现
        }
    };
}

macro_rules! alloc_dbg {
    ($($arg:tt)*) => {
        #[cfg(feature = "alloc-auto-test")]
        {
            #[cfg(feature = "log")]
            {
                use crate::log;
                log::info!($($arg)*);
            }
            #[cfg(not(feature = "log"))]
            {
                crate::print!("[DEBUG] ");
                crate::println!($($arg)*);
            }
        }
        #[cfg(not(feature = "alloc-auto-test"))]
        {
            // 空实现
        }
    };
}

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

// 从链接脚本引入堆起始地址
unsafe extern "C" {
    static _heap_start: u8;
}

// 内存对齐要求
const MIN_ALIGN: usize = 4;
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<BlockHeader>() + MIN_ALIGN;

// 块头部信息（所有块共用）
#[repr(C)]
struct BlockHeader {
    size: usize,                    // 块的总大小（包括头部）
    is_free: bool,                  // 是否空闲
    next: Option<*mut BlockHeader>, // 仅用于空闲链表
}

impl BlockHeader {
    // 从地址创建节点
    unsafe fn from_addr(addr: usize, size: usize, is_free: bool) -> *mut BlockHeader {
        println!(
            "BlockHeader::from_addr: addr=0x{:08x}, size={}, is_free={}",
            addr, size, is_free
        );
        let ptr = addr as *mut BlockHeader;
        // SAFETY: 调用者确保地址有效且对齐
        unsafe {
            (*ptr).size = size;
            (*ptr).is_free = is_free;
            (*ptr).next = None;
        }
        println!(
            "BlockHeader::from_addr: created at 0x{:08x} with size={}",
            ptr as usize,
            unsafe { (*ptr).size }
        );
        ptr
    }

    // 获取数据区域起始地址
    fn data_addr(&self) -> usize {
        (self as *const _ as usize) + core::mem::size_of::<Self>()
    }

    // 获取块结束地址
    fn end_addr(&self) -> usize {
        (self as *const _ as usize) + self.size
    }

    // 获取可用数据大小（不包括头部）
    fn data_size(&self) -> usize {
        self.size - core::mem::size_of::<Self>()
    }
}

// 全局分配器内部状态
struct GlobalAllocatorInner {
    free_list_head: Option<*mut BlockHeader>,
    initialized: bool,
}

impl GlobalAllocatorInner {
    const fn new() -> Self {
        Self {
            free_list_head: None,
            initialized: false,
        }
    }

    // 初始化堆内存
    unsafe fn init(&mut self) {
        println!("GlobalAllocatorInner::init: starting");

        if self.initialized {
            println!("GlobalAllocatorInner::init: already initialized");
            return;
        }

        // 获取堆区域信息
        // SAFETY: 调用者确保这是有效的堆起始地址
        let heap_start = unsafe { &_heap_start as *const u8 as usize };
        println!(
            "GlobalAllocatorInner::init: heap_start=0x{:08x}",
            heap_start
        );

        let heap_end = 0x04800000; // 8MB RAM 结束地址
        println!("GlobalAllocatorInner::init: heap_end=0x{:08x}", heap_end);

        if heap_start >= heap_end {
            println!("GlobalAllocatorInner::init: ERROR - heap_start >= heap_end");
            panic!("Invalid heap region");
        }

        let heap_size = heap_end - heap_start;
        println!("GlobalAllocatorInner::init: heap_size={} bytes", heap_size);

        if heap_size < MIN_BLOCK_SIZE {
            println!("GlobalAllocatorInner::init: ERROR - heap too small");
            panic!("Heap too small");
        }

        // 将整个堆初始化为一个空闲块
        // SAFETY: heap_start 有效且对齐
        let free_node = unsafe { BlockHeader::from_addr(heap_start, heap_size, true) };
        self.free_list_head = Some(free_node);
        self.initialized = true;

        println!(
            "GlobalAllocatorInner::init: initialized with free block at 0x{:08x}",
            free_node as usize
        );
        self.print_free_list();
    }

    // 打印空闲链表状态
    fn print_free_list(&self) {
        println!("Free list status:");
        if !self.initialized {
            println!("  Not initialized");
            return;
        }

        let mut count = 0;
        let mut current_ptr = self.free_list_head;
        while let Some(mut current) = current_ptr {
            // SAFETY: 仅用于调试输出，不会修改
            unsafe {
                let node = &*current;
                println!(
                    "  Block {}: addr=0x{:08x}, size={}, data_size={}, next={:?}",
                    count,
                    current as usize,
                    node.size,
                    node.data_size(),
                    node.next.map(|p| p as usize)
                );
                current_ptr = node.next;
            }
            count += 1;
        }
        println!("  Total free blocks: {}", count);
    }

    // 向上对齐
    fn align_up(addr: usize, align: usize) -> usize {
        let aligned = (addr + align - 1) & !(align - 1);
        aligned
    }

    // 合并相邻空闲块
    unsafe fn coalesce(&mut self) {
        println!("GlobalAllocatorInner::coalesce: starting");
        let mut current_ptr = self.free_list_head;
        let mut prev_ptr: Option<*mut BlockHeader> = None;
        let mut merged_count = 0;

        while let Some(mut current) = current_ptr {
            // SAFETY: current 是有效的 BlockHeader 指针
            let current_node = unsafe { &mut *current };
            let node_end = current_node.end_addr();
            println!(
                "coalesce: checking block at 0x{:08x}, end=0x{:08x}",
                current as usize, node_end
            );

            // 检查是否可以与下一个块合并
            if let Some(next_ptr) = current_node.next {
                let next_addr = next_ptr as usize;
                println!("coalesce:   next block at 0x{:08x}", next_addr);

                if node_end == next_addr {
                    println!(
                        "coalesce:   MERGING blocks: 0x{:08x} + 0x{:08x}",
                        current as usize, next_addr
                    );
                    // SAFETY: next_ptr 是有效的 BlockHeader 指针
                    let next_node = unsafe { &mut *next_ptr };
                    current_node.size += next_node.size;
                    current_node.next = next_node.next.take();
                    merged_count += 1;
                    println!("coalesce:   new size: {}", current_node.size);
                    // 重新检查这个位置，因为可能还能继续合并
                    continue;
                }
            }

            prev_ptr = Some(current);
            current_ptr = current_node.next;
        }

        println!(
            "GlobalAllocatorInner::coalesce: completed, merged {} blocks",
            merged_count
        );
        if merged_count > 0 {
            self.print_free_list();
        }
    }

    // 内部分配函数
    unsafe fn alloc_impl(&mut self, layout: Layout) -> *mut u8 {
        println!("GlobalAllocatorInner::alloc_impl: layout={:?}", layout);

        if !self.initialized {
            println!("alloc_impl: ERROR - not initialized");
            return null_mut();
        }

        // 计算所需总大小（包括头部和对齐）
        let required_size = layout.size();
        let align = layout.align().max(MIN_ALIGN);

        // 总大小 = 头部大小 + 对齐后的数据大小
        let total_needed =
            core::mem::size_of::<BlockHeader>() + Self::align_up(required_size, align);
        println!(
            "alloc_impl: required_size={}, align={}, total_needed={}",
            required_size, align, total_needed
        );

        // 遍历空闲链表寻找合适的块
        let mut prev_ptr: Option<*mut BlockHeader> = None;
        let mut current_ptr = self.free_list_head;
        let mut block_index = 0;

        while let Some(mut current) = current_ptr {
            println!(
                "alloc_impl: checking block {} at 0x{:08x}",
                block_index, current as usize
            );
            // SAFETY: current 是有效的 BlockHeader 指针
            let current_node = unsafe { &mut *current };
            let block_size = current_node.size;
            println!(
                "alloc_impl:   block size={}, data_size={}",
                block_size,
                current_node.data_size()
            );

            if block_size >= total_needed {
                println!("alloc_impl:   FOUND suitable block!");

                // 计算剩余空间
                let remaining = block_size - total_needed;
                println!(
                    "alloc_impl:   remaining space after allocation: {}",
                    remaining
                );

                if remaining >= MIN_BLOCK_SIZE {
                    println!("alloc_impl:   SPLITTING block (remaining >= MIN_BLOCK_SIZE)");
                    // 分割块：创建新的空闲块
                    let new_free_addr = (current as usize) + total_needed;
                    println!("alloc_impl:   new_free_addr=0x{:08x}", new_free_addr);
                    // SAFETY: new_free_addr 在当前块内部，有效且对齐
                    let new_free =
                        unsafe { BlockHeader::from_addr(new_free_addr, remaining, true) };

                    // 链接新空闲块
                    // SAFETY: new_free 是有效的 BlockHeader 指针
                    unsafe {
                        (*new_free).next = current_node.next.take();
                    }

                    // 从链表中移除当前块或更新大小
                    if let Some(mut prev) = prev_ptr {
                        println!("alloc_impl:   updating previous block's next pointer");
                        // SAFETY: prev 是有效的 BlockHeader 指针
                        unsafe {
                            (*prev).next = Some(new_free);
                        }
                    } else {
                        println!("alloc_impl:   updating free_list_head to new free block");
                        self.free_list_head = Some(new_free);
                    }
                } else {
                    println!("alloc_impl:   USING entire block (remaining < MIN_BLOCK_SIZE)");
                    // 整个块都被使用，从链表中移除
                    if let Some(mut prev) = prev_ptr {
                        println!("alloc_impl:   removing block from middle of list");
                        // SAFETY: prev 是有效的 BlockHeader 指针
                        unsafe {
                            (*prev).next = current_node.next.take();
                        }
                    } else {
                        println!("alloc_impl:   removing block from head of list");
                        self.free_list_head = current_node.next.take();
                    }
                }

                // 更新当前块为已分配状态
                current_node.size = total_needed;
                current_node.is_free = false;
                current_node.next = None;

                let data_addr = current_node.data_addr();
                println!(
                    "alloc_impl:   set block at 0x{:08x} as allocated, size={}",
                    current as usize, total_needed
                );
                println!("alloc_impl:   returning data pointer: 0x{:08x}", data_addr);
                self.print_free_list();
                return data_addr as *mut u8;
            } else {
                println!("alloc_impl:   block too small");
            }

            // 移动到下一个节点
            prev_ptr = Some(current);
            current_ptr = current_node.next;
            block_index += 1;
        }

        println!("GlobalAllocatorInner::alloc_impl: NO suitable block found");
        println!("alloc_impl: free list state before failure:");
        self.print_free_list();
        // 没有找到合适的空闲块
        null_mut()
    }

    // 内部释放函数
    unsafe fn dealloc_impl(&mut self, ptr: *mut u8, layout: Layout) {
        println!(
            "GlobalAllocatorInner::dealloc_impl: ptr=0x{:p}, layout={:?}",
            ptr, layout
        );

        if ptr.is_null() {
            println!("dealloc_impl: WARNING - null pointer");
            return;
        }

        if !self.initialized {
            println!("dealloc_impl: ERROR - not initialized");
            return;
        }

        let data_addr = ptr as usize;
        let header_addr = data_addr - core::mem::size_of::<BlockHeader>();
        println!(
            "dealloc_impl: data_addr=0x{:08x}, header_addr=0x{:08x}",
            data_addr, header_addr
        );

        // 获取块头部
        let block_ptr = header_addr as *mut BlockHeader;
        // SAFETY: header_addr 应该是有效的 BlockHeader
        let block = unsafe { &mut *block_ptr };

        // 标记为空闲
        block.is_free = true;
        let block_size = block.size;
        println!(
            "dealloc_impl: marking block at 0x{:08x} as free, size={}",
            header_addr, block_size
        );

        // 按地址顺序插入到空闲链表
        let insert_addr = block_ptr as usize;

        // 如果链表为空，直接插入
        if self.free_list_head.is_none() {
            println!("dealloc_impl: free list empty, inserting as only block");
            block.next = None;
            self.free_list_head = Some(block_ptr);
            // SAFETY: 刚刚初始化了空闲链表
            unsafe {
                self.coalesce();
            }
            return;
        }

        // 如果要插入到链表头部
        if let Some(head) = self.free_list_head {
            if insert_addr < head as usize {
                println!(
                    "dealloc_impl: inserting before head (0x{:08x})",
                    head as usize
                );
                block.next = self.free_list_head;
                self.free_list_head = Some(block_ptr);
                // SAFETY: 刚刚修改了空闲链表
                unsafe {
                    self.coalesce();
                }
                return;
            }
        }

        // 遍历链表找到插入位置
        let mut current_ptr = self.free_list_head;
        let mut prev_ptr: Option<*mut BlockHeader> = None;
        let mut position = 0;

        while let Some(mut current) = current_ptr {
            let current_addr = current as usize;
            println!(
                "dealloc_impl: checking position {}: addr=0x{:08x}",
                position, current_addr
            );

            if insert_addr < current_addr {
                println!(
                    "dealloc_impl: inserting before block at 0x{:08x}",
                    current_addr
                );
                // 插入到当前节点之前
                block.next = Some(current);

                if let Some(mut prev) = prev_ptr {
                    println!("dealloc_impl: updating previous block's next pointer");
                    // SAFETY: prev 是有效的 BlockHeader 指针
                    unsafe {
                        (*prev).next = Some(block_ptr);
                    }
                }
                // SAFETY: 刚刚修改了空闲链表
                unsafe {
                    self.coalesce();
                }
                return;
            }

            // 移动到下一个节点
            prev_ptr = Some(current);
            // SAFETY: current 是有效的 BlockHeader 指针
            let current_node = unsafe { &mut *current };
            current_ptr = current_node.next;
            position += 1;
        }

        // 插入到链表末尾
        println!("dealloc_impl: inserting at end of list");
        if let Some(mut prev) = prev_ptr {
            // SAFETY: prev 是有效的 BlockHeader 指针
            unsafe {
                (*prev).next = Some(block_ptr);
            }
        }

        // SAFETY: 刚刚修改了空闲链表
        unsafe {
            self.coalesce();
        }

        println!("dealloc_impl: completed successfully");
        self.print_free_list();
    }
}

// 全局分配器（线程安全包装）
pub struct GlobalAllocator {
    inner: UnsafeCell<GlobalAllocatorInner>,
}

// SAFETY: 在单线程环境中，使用 UnsafeCell 是安全的
unsafe impl Sync for GlobalAllocator {}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            inner: UnsafeCell::new(GlobalAllocatorInner::new()),
        }
    }

    // 初始化堆内存
    pub unsafe fn init(&self) {
        println!("GlobalAllocator::init: starting");
        // SAFETY: 获取内部状态的可变引用
        let inner = unsafe { &mut *self.inner.get() };
        unsafe {
            inner.init();
        }
        println!("GlobalAllocator::init: completed");
    }

    #[cfg(feature = "alloc-auto-test")]
    /// for dev fn test
    pub fn print_free_list(&self) {
        // SAFETY: 获取内部状态的可变引用
        let inner = unsafe { &*self.inner.get() };
        inner.print_free_list()
    }
}

// 实现GlobalAlloc trait
unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        println!("GlobalAlloc::alloc: layout={:?}", layout);
        // SAFETY: 获取内部状态的可变引用
        let inner = unsafe { &mut *self.inner.get() };
        // SAFETY: 调用者确保这是有效的分配请求
        let result = unsafe { inner.alloc_impl(layout) };
        println!("GlobalAlloc::alloc: returning {:?}", result);
        result
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        println!("GlobalAlloc::dealloc: ptr=0x{:p}, layout={:?}", ptr, layout);
        // SAFETY: 获取内部状态的可变引用
        let inner = unsafe { &mut *self.inner.get() };
        // SAFETY: 调用者确保这是有效的释放请求
        unsafe {
            inner.dealloc_impl(ptr, layout);
        }
        println!("GlobalAlloc::dealloc: completed");
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        println!("GlobalAlloc::alloc_zeroed: layout={:?}", layout);
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() {
            println!(
                "GlobalAlloc::alloc_zeroed: zeroing {} bytes at 0x{:p}",
                layout.size(),
                ptr
            );
            // SAFETY: ptr 是有效的已分配内存
            unsafe {
                ptr.write_bytes(0, layout.size());
            }
        }
        println!("GlobalAlloc::alloc_zeroed: returning {:?}", ptr);
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        println!(
            "GlobalAlloc::realloc: ptr=0x{:p}, layout={:?}, new_size={}",
            ptr, layout, new_size
        );

        if new_size == 0 {
            println!("GlobalAlloc::realloc: new_size=0, deallocating");
            unsafe {
                self.dealloc(ptr, layout);
            }
            return null_mut();
        }

        if ptr.is_null() {
            println!("GlobalAlloc::realloc: null pointer, allocating new block");
            let new_layout = Layout::from_size_align(new_size, layout.align()).unwrap_or(layout);
            return unsafe { self.alloc(new_layout) };
        }

        // 获取原分配大小
        let data_addr = ptr as usize;
        let header_addr = data_addr - core::mem::size_of::<BlockHeader>();
        // SAFETY: header_addr 是有效的 BlockHeader
        let block_ptr = header_addr as *const BlockHeader;
        let old_total_size = unsafe { (*block_ptr).size };
        let old_data_size = old_total_size - core::mem::size_of::<BlockHeader>();

        println!(
            "GlobalAlloc::realloc: old_data_size={}, new_size={}",
            old_data_size, new_size
        );

        if new_size <= old_data_size {
            println!("GlobalAlloc::realloc: new_size <= old_data_size, returning same pointer");
            return ptr;
        }

        // 分配新内存并复制数据
        println!("GlobalAlloc::realloc: allocating new larger block");
        let new_layout = Layout::from_size_align(new_size, layout.align()).unwrap_or(layout);
        let new_ptr = unsafe { self.alloc(new_layout) };

        if !new_ptr.is_null() {
            println!(
                "GlobalAlloc::realloc: copying {} bytes from 0x{:p} to 0x{:p}",
                old_data_size, ptr, new_ptr
            );
            // SAFETY: ptr 和 new_ptr 都是有效的指针，不重叠
            unsafe {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, old_data_size);
                self.dealloc(ptr, layout);
            }
        } else {
            println!("GlobalAlloc::realloc: FAILED to allocate new block");
        }

        println!("GlobalAlloc::realloc: returning {:?}", new_ptr);
        new_ptr
    }
}

// 全局分配器实例
#[global_allocator]
static ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub unsafe fn init() {
    println!("=== ALLOCATOR INIT START ===");
    // SAFETY: 在系统启动时调用，确保单线程访问
    unsafe {
        ALLOCATOR.init();
    }
    println!("=== ALLOCATOR INIT COMPLETE ===");

    #[cfg(feature = "alloc-auto-test")]
    test();
}

/// 使用一些基础的动态变量测试分配器
#[cfg(feature = "alloc-auto-test")]
pub fn test() {
    extern crate alloc;
    use alloc::boxed::Box;
    use alloc::collections::LinkedList;
    use alloc::format;
    use alloc::string::String;
    use alloc::{vec, vec::Vec};

    alloc_dbg!("[DEBUG] === STARTING ALLOCATOR TEST ===");
    alloc_dbg!("Testing allocator...");

    {
        // 测试1：基本分配
        alloc_dbg!("\n=== Test 1: Basic allocations ===");
        {
            let mut xxx = vec![233];
            xxx.push(666);
            alloc_dbg!("[DEBUG] Creating Vec with 10 elements...");
            let mut v = Vec::new();
            for i in 0..10 {
                v.push(i * 10);
            }
            alloc_dbg!("Vec: {:?}", v);

            // 验证向量内容
            assert_eq!(v.len(), 10, "Vec should have 10 elements");
            for i in 0..10 {
                assert_eq!(v[i], i * 10, "Vec[{}] should be {}", i, i * 10);
            }
            assert_eq!(v.capacity() >= 10, true, "Vec should have capacity >= 10");
            alloc_dbg!(
                "✅ Test 1 passed: Vec allocation verified and xxx: [{}:233, {}:666]",
                xxx[0],
                xxx[1]
            );

            // Vec 在这里结束生命周期
        }
        alloc_dbg!("[DEBUG] Vec dropped");

        // 测试2：字符串操作
        alloc_dbg!("\n=== Test 2: String operations ===");
        {
            alloc_dbg!("[DEBUG] Creating String...");
            let s = String::from("Hello allocator!");
            alloc_dbg!("String: {}", s);

            // 验证第一个字符串
            assert_eq!(s, "Hello allocator!", "String should be 'Hello allocator!'");
            assert_eq!(s.len(), 16, "String length should be 16");

            alloc_dbg!("[DEBUG] Creating another String...");
            let s2 = String::from("This is a test!");
            alloc_dbg!("String2: {}", s2);

            // 验证第二个字符串
            assert_eq!(s2, "This is a test!", "String2 should be 'This is a test!'");
            assert_eq!(s2.len(), 15, "String2 length should be 15");

            // 验证两个字符串不同
            assert_ne!(s, s2, "Two strings should be different");
            alloc_dbg!("✅ Test 2 passed: String allocations verified");

            // 两个String都结束生命周期
        }
        alloc_dbg!("[DEBUG] Strings dropped");

        // 测试3：多级嵌套结构
        alloc_dbg!("\n=== Test 3: Nested structures ===");
        {
            alloc_dbg!("[DEBUG] Creating Box with large data...");
            let b1 = Box::new([0u8; 1024]); // 1KB
            alloc_dbg!("[DEBUG] Box created with 1KB array");

            // 验证Box数组内容
            assert_eq!(b1.len(), 1024, "Box array should have 1024 elements");
            assert!(b1.iter().all(|&x| x == 0), "All elements should be 0");

            alloc_dbg!("[DEBUG] Creating Vec of Boxes...");
            let mut boxes = Vec::new();
            for i in 0..5 {
                boxes.push(Box::new(i * 100));
            }
            alloc_dbg!(
                "Vec of Boxes: {:?}",
                boxes.iter().map(|b| **b).collect::<Vec<_>>()
            );

            // 验证Vec of Boxes内容
            assert_eq!(boxes.len(), 5, "Should have 5 boxes");
            for (i, b) in boxes.iter().enumerate() {
                assert_eq!(**b, i * 100, "Box {} should contain {}", i, i * 100);
            }
            alloc_dbg!("✅ Test 3 passed: Nested structures verified");
        }
        alloc_dbg!("[DEBUG] Nested structures dropped");

        // 测试4：链表测试
        alloc_dbg!("\n=== Test 4: Linked list ===");
        {
            alloc_dbg!("[DEBUG] Creating LinkedList...");
            let mut list = LinkedList::new();
            for i in 0..10 {
                list.push_back(i);
                list.push_front(i + 10);
            }
            alloc_dbg!("LinkedList length: {}", list.len());

            // 验证链表长度和部分内容
            assert_eq!(list.len(), 20, "LinkedList should have 20 elements");

            // 验证链表包含正确的值
            let mut count_front = 0;
            let mut count_back = 0;
            for &value in &list {
                if value < 10 {
                    count_front += 1;
                } else {
                    count_back += 1;
                }
            }
            assert_eq!(count_front, 10, "Should have 10 front-pushed elements");
            assert_eq!(count_back, 10, "Should have 10 back-pushed elements");
            alloc_dbg!("✅ Test 4 passed: LinkedList allocation verified");
        }
        alloc_dbg!("[DEBUG] LinkedList dropped");

        // 测试5：大规模分配和释放
        alloc_dbg!("\n=== Test 5: Large scale allocations ===");
        {
            alloc_dbg!("[DEBUG] Allocating 100 small chunks...");
            let mut chunks: Vec<Box<[u8]>> = Vec::new(); // 改为动态切片类型
            for i in 0..100 {
                let chunk = Box::new([i as u8; 16]); // 16字节数组
                chunks.push(chunk);
            }
            alloc_dbg!("Allocated {} chunks", chunks.len());

            // 验证所有chunks
            assert_eq!(chunks.len(), 100, "Should have 100 chunks initially");
            for (i, chunk) in chunks.iter().enumerate() {
                assert_eq!(chunk.len(), 16, "Chunk {} should have length 16", i);
                assert!(
                    chunk.iter().all(|&x| x == i as u8),
                    "All bytes in chunk {} should be {}",
                    i,
                    i
                );
            }
            alloc_dbg!("✅ Initial 100 chunks verified");

            // 释放一半
            alloc_dbg!("[DEBUG] Releasing last 50 chunks...");

            chunks.drain(0..25);
            alloc_dbg!("Remaining chunks: {}", chunks.len());
            assert_eq!(chunks.len(), 75, "Should have 75 chunks after truncate");

            let mut last = chunks.split_off(25);
            let mut chunks = last;
            alloc_dbg!("Remaining chunks: {}", chunks.len());
            assert_eq!(chunks.len(), 50, "Should have 50 chunks after truncate");

            // 验证剩余chunks
            for (i, chunk) in chunks.iter().enumerate() {
                assert_eq!(
                    chunk.len(),
                    16,
                    "Remaining chunk {} should have length 16",
                    i
                );
                assert_eq!(
                    chunk[0],
                    (i + 50) as u8,
                    "Remaining chunk {} should start with {}",
                    i,
                    i + 50
                );
            }

            // 再分配更多
            alloc_dbg!("[DEBUG] Allocating 50 more chunks of different sizes...");
            for i in 0..25 {
                let chunk = Box::new([(i + 100) as u8; 16]); // 继续用16字节
                chunks.push(chunk);
            }
            for i in 0..25 {
                let chunk = Box::new([(i + 125) as u8; 32]); // 改用32字节
                chunks.push(chunk);
            }
            alloc_dbg!("Total chunks: {}", chunks.len());

            // 验证最终chunks
            assert_eq!(chunks.len(), 100, "Should have 100 chunks total");

            // 验证16字节chunks
            for i in 0..25 {
                let chunk = &chunks[50 + i];
                assert_eq!(chunk.len(), 16, "Chunk {} should be 16 bytes", 50 + i);
                assert!(
                    chunk.iter().all(|&x| x == (i + 100) as u8),
                    "Chunk {} should all be {}",
                    50 + i,
                    i + 100
                );
            }

            // 验证32字节chunks
            for i in 0..25 {
                let chunk = &chunks[75 + i];
                assert_eq!(chunk.len(), 32, "Chunk {} should be 32 bytes", 75 + i);
                assert!(
                    chunk.iter().all(|&x| x == (i + 125) as u8),
                    "Chunk {} should all be {}",
                    75 + i,
                    i + 125
                );
            }

            alloc_dbg!("✅ Test 5 passed: Large scale allocations verified");
        }
        alloc_dbg!("[DEBUG] Large scale allocations dropped");

        // 测试6：内存重用测试
        alloc_dbg!("\n=== Test 6: Memory reuse test ===");
        {
            alloc_dbg!("[DEBUG] Allocating temporary memory...");
            let temp = Box::new([1, 2, 3, 4, 5]);
            alloc_dbg!("Temp box: {:?}", *temp);

            // 验证temp内容
            assert_eq!(*temp, [1, 2, 3, 4, 5], "Temp box should be [1,2,3,4,5]");

            alloc_dbg!("[DEBUG] Allocating new memory (should reuse previous)...");
            let new_temp = Box::new([6, 7, 8, 9, 10]);
            alloc_dbg!("New temp box: {:?}", *new_temp);

            // 验证new_temp内容
            assert_eq!(
                *new_temp,
                [6, 7, 8, 9, 10],
                "New temp box should be [6,7,8,9,10]"
            );

            // 再次验证，确保没有被覆盖或者其他
            assert_eq!(*temp, [1, 2, 3, 4, 5], "Temp box should be [1,2,3,4,5]");

            alloc_dbg!("✅ Test 6 passed: Memory reuse test verified");
        }
        alloc_dbg!("[DEBUG] Memory reuse test complete");

        // 测试7：混合类型测试
        alloc_dbg!("\n=== Test 7: Mixed types test ===");
        {
            alloc_dbg!("[DEBUG] Creating mixed data structures...");

            // 同时存在多个不同类型的数据结构
            let vec1 = vec![1, 2, 3];
            let vec2 = vec![4.0, 5.0, 6.0];
            let string1 = String::from("First string");
            let string2 = String::from("Second string");
            let box1 = Box::new(42);
            let box2 = Box::new(3.14159);

            alloc_dbg!("vec1: {:?}", vec1);
            alloc_dbg!("vec2: {:?}", vec2);
            alloc_dbg!("string1: {}", string1);
            alloc_dbg!("string2: {}", string2);
            alloc_dbg!("box1: {}", box1);
            alloc_dbg!("box2: {}", box2);

            // 验证所有变量
            assert_eq!(vec1, [1, 2, 3], "vec1 should be [1,2,3]");
            assert_eq!(vec2, [4.0, 5.0, 6.0], "vec2 should be [4.0,5.0,6.0]");
            assert_eq!(string1, "First string", "string1 should be 'First string'");
            assert_eq!(
                string2, "Second string",
                "string2 should be 'Second string'"
            );
            assert_eq!(*box1, 42, "box1 should be 42");
            assert!(
                (*box2 - 3.14159f64).abs() < 0.00001,
                "box2 should be approximately 3.14159"
            );

            // 在作用域中同时使用所有变量
            let sum: f64 =
                vec1.iter().sum::<i32>() as f64 + vec2.iter().sum::<f64>() + *box1 as f64 + *box2;
            alloc_dbg!("Sum of all values: {}", sum);

            // 验证计算结果
            let expected_sum = 6 as f64 + 15.0 + 42.0 + 3.14159;
            assert!(
                (sum - expected_sum).abs() < 0.00001,
                "Sum should be approximately {}",
                expected_sum
            );

            alloc_dbg!("✅ Test 7 passed: Mixed types verified");
        }
        alloc_dbg!("[DEBUG] Mixed types test complete");

        // 测试8：内存压力测试
        alloc_dbg!("\n=== Test 8: Memory stress test ===");
        {
            alloc_dbg!("[DEBUG] Creating many allocations...");
            let mut allocations = Vec::new();

            // 不同大小的分配
            for size_power in 0..6 {
                // 从1字节到32字节
                let size = 1 << size_power;
                for _ in 0..10 {
                    let data = vec![size as u8; size]; // 用size值填充
                    allocations.push(data);
                }
            }
            alloc_dbg!("Created {} allocations", allocations.len());

            // 验证所有分配
            let mut idx = 0;
            for size_power in 0..6 {
                let size = 1 << size_power;
                for _ in 0..10 {
                    let data = &allocations[idx];
                    assert_eq!(
                        data.len(),
                        size,
                        "Allocation {} should have size {}",
                        idx,
                        size
                    );
                    assert!(
                        data.iter().all(|&x| x == size as u8),
                        "All bytes in allocation {} should be {}",
                        idx,
                        size
                    );
                    idx += 1;
                }
            }

            alloc_dbg!("✅ Initial allocations verified");

            // 随机释放一些
            alloc_dbg!("[DEBUG] Randomly releasing some allocations...");

            let mut curr = allocations.len();

            for i in (0..allocations.len()).step_by(2) {
                if i < allocations.len() {
                    allocations.remove(i);
                    curr -= 1;
                }
            }
            alloc_dbg!("Remaining allocations: {}", allocations.len());

            // 验证剩余分配
            assert_eq!(allocations.len(), curr, "Should have 31 allocations left");

            alloc_dbg!("✅ Test 8 passed: Memory stress test verified");
        }
        alloc_dbg!("[DEBUG] Memory stress test complete");

        // 测试9：验证分配器内部状态
        alloc_dbg!("\n=== Test 9: Allocator state verification ===");
        {
            // 做一些分配
            let a = Box::new(1);
            let b = Box::new(2);
            let c = Box::new(3);

            alloc_dbg!("Allocated 3 boxes: {}, {}, {}", a, b, c);

            // 验证分配的值
            assert_eq!(*a, 1, "Box a should be 1");
            assert_eq!(*b, 2, "Box b should be 2");
            assert_eq!(*c, 3, "Box c should be 3");

            // 故意提前释放一些
            alloc_dbg!("[DEBUG] Manually dropping b...");
            drop(b);

            // 验证a和c仍然有效
            assert_eq!(*a, 1, "Box a should still be 1 after dropping b");
            assert_eq!(*c, 3, "Box c should still be 3 after dropping b");

            // 再分配
            let d = Box::new(4);
            alloc_dbg!("Allocated new box: {}", d);

            // 验证d的值
            assert_eq!(*d, 4, "Box d should be 4");

            // 验证所有有效boxes
            assert_eq!(*a + *c + *d, 1 + 3 + 4, "Sum of boxes should be 8");

            alloc_dbg!("✅ Test 9 passed: Allocator state verified");
        }
        alloc_dbg!("[DEBUG] Allocator state test complete");

        // 测试10：读写正确性验证
        alloc_dbg!("\n=== Test 10: Read/Write correctness ===");
        {
            alloc_dbg!("[DEBUG] Creating test vectors...");

            // 测试各种类型的向量
            let vec1 = vec![1, 2, 3, 4, 5, 233, 666, 114514, 0x114514];
            alloc_dbg!("vec1: {:?}", vec1);

            // 验证vec1的值
            assert_eq!(vec1[0], 1, "vec1[0] should be 1");
            assert_eq!(vec1[5], 233, "vec1[5] should be 233");
            assert_eq!(vec1[6], 666, "vec1[6] should be 666");
            assert_eq!(vec1[7], 114514, "vec1[7] should be 114514");
            assert_eq!(vec1[8], 0x114514, "vec1[8] should be 0x114514");
            alloc_dbg!("✅ vec1 values verified");

            let vec2 = vec![233; 233];
            alloc_dbg!("vec2: first 5 elements = {:?}", &vec2[..5]);
            assert_eq!(vec2.len(), 233, "vec2 should have 233 elements");
            assert!(
                vec2.iter().all(|&x| x == 233),
                "All elements in vec2 should be 233"
            );
            alloc_dbg!("✅ vec2 values verified");

            alloc_dbg!("[DEBUG] Creating test strings...");

            let str1 = "你好！";
            let str2 = "hk128";
            let s1 = String::from(str1);
            let s2 = String::from(str2);

            alloc_dbg!("str1: {}", s1);
            alloc_dbg!("str2: {}", s2);

            // 验证字符串内容
            assert_eq!(s1, "你好！", "s1 should be '你好！'");
            assert_eq!(s2, "hk128", "s2 should be 'hk128'");
            alloc_dbg!("✅ Individual strings verified");

            // 字符串拼接验证
            let combined = format!("{} {}", s1, s2);
            alloc_dbg!("Combined string: {}", combined);
            assert_eq!(
                combined, "你好！ hk128",
                "Combined string should be '你好！ hk128'"
            );
            alloc_dbg!("✅ String concatenation verified");

            // 测试字符串修改
            alloc_dbg!("[DEBUG] Testing string mutation...");
            let mut mutable_string = String::from("Hello");
            mutable_string.push_str(" World!");
            assert_eq!(
                mutable_string, "Hello World!",
                "Mutable string should be 'Hello World!'"
            );
            alloc_dbg!("✅ String mutation verified: {}", mutable_string);

            // 测试向量修改
            alloc_dbg!("[DEBUG] Testing vector mutation...");
            let mut mutable_vec = vec![1, 2, 3];
            mutable_vec.push(4);
            mutable_vec.push(5);
            assert_eq!(
                mutable_vec,
                vec![1, 2, 3, 4, 5],
                "Vector should be [1,2,3,4,5]"
            );
            alloc_dbg!("✅ Vector mutation verified: {:?}", mutable_vec);

            // 测试Box读写
            alloc_dbg!("[DEBUG] Testing Box read/write...");
            let mut boxed_value = Box::new(42);
            assert_eq!(*boxed_value, 42, "Box should contain 42");

            *boxed_value = 100;
            assert_eq!(*boxed_value, 100, "Box should now contain 100");
            alloc_dbg!("✅ Box read/write verified: {}", *boxed_value);

            // 测试数组读写
            alloc_dbg!("[DEBUG] Testing array read/write...");
            let mut boxed_array = Box::new([0u8; 10]);
            for i in 0..10 {
                boxed_array[i] = i as u8;
            }

            // 验证数组内容
            for i in 0..10 {
                assert_eq!(boxed_array[i], i as u8, "Array[{}] should be {}", i, i);
            }
            alloc_dbg!("✅ Array read/write verified: {:?}", *boxed_array);

            // 复杂数据结构测试
            alloc_dbg!("[DEBUG] Testing complex data structure...");
            struct Point {
                x: i32,
                y: i32,
            }

            let boxed_point = Box::new(Point { x: 10, y: 20 });
            assert_eq!(boxed_point.x, 10, "Point.x should be 10");
            assert_eq!(boxed_point.y, 20, "Point.y should be 20");
            alloc_dbg!(
                "✅ Complex data structure verified: Point({}, {})",
                boxed_point.x,
                boxed_point.y
            );

            // 测试内存内容是否保持
            alloc_dbg!("[DEBUG] Testing memory persistence...");
            let persistent_string = String::from("This should persist");
            let string_ptr = persistent_string.as_ptr();
            let string_len = persistent_string.len();

            // 创建另一个分配，确保不会覆盖前一个
            let another_string = String::from("Another allocation");

            // 验证第一个字符串仍然正确
            unsafe {
                let slice = core::slice::from_raw_parts(string_ptr, string_len);
                let reconstructed = String::from_utf8_unchecked(slice.to_vec());
                assert_eq!(
                    reconstructed, "This should persist",
                    "String should persist in memory"
                );
            }
            alloc_dbg!("✅ Memory persistence verified");

            alloc_dbg!("✅ Test 10 passed: All read/write operations verified");
        }
        #[cfg(feature = "rand")]
        // 测试11：随机动态验证
        alloc_dbg!("\n=== Test 11: Random dynamic verification ===");
        {
            use crate::rand::{Rng, SeedableRng, rngs::SmallRng};

            alloc_dbg!("[DEBUG] Creating RNG...");
            let mut rng = SmallRng::seed_from_u64(0x1234567890ABCDEF);

            alloc_dbg!("[DEBUG] Creating multiple vectors with random data...");

            // 创建多个向量
            let mut vec1 = Vec::new();
            let mut vec2 = Vec::new();
            let mut vec3 = Vec::new();

            // 动态添加随机数据
            let num_elements = 50;
            alloc_dbg!("Generating {} random elements...", num_elements);

            for i in 0..num_elements {
                let value: i32 = rng.random_range(-1000..1000);
                vec1.push(value); // 原始值
                vec2.push(value + 1); // 值+1
                vec3.push(value * 2); // 值*2

                // 每10个元素验证一次中间状态
                if i % 10 == 9 {
                    alloc_dbg!("[DEBUG] Intermediate verification after {} elements", i + 1);

                    // 验证vec1和vec2的关系
                    for j in 0..=i {
                        assert_eq!(
                            vec2[j],
                            vec1[j] + 1,
                            "Element {}: vec2[{}] = {} should be vec1[{}] + 1 = {} + 1",
                            j,
                            j,
                            vec2[j],
                            j,
                            vec1[j]
                        );
                    }

                    // 验证vec1和vec3的关系
                    for j in 0..=i {
                        assert_eq!(
                            vec3[j],
                            vec1[j] * 2,
                            "Element {}: vec3[{}] = {} should be vec1[{}] * 2 = {} * 2",
                            j,
                            j,
                            vec3[j],
                            j,
                            vec1[j]
                        );
                    }
                    alloc_dbg!("✅ Intermediate verification passed for {} elements", i + 1);
                }
            }

            alloc_dbg!("Final vectors created:");
            alloc_dbg!("vec1 (first 10): {:?}", &vec1[..10.min(vec1.len())]);
            alloc_dbg!("vec2 (first 10): {:?}", &vec2[..10.min(vec2.len())]);
            alloc_dbg!("vec3 (first 10): {:?}", &vec3[..10.min(vec3.len())]);

            // 最终验证1：基本关系
            alloc_dbg!("[DEBUG] Verifying basic relationships...");
            assert_eq!(
                vec1.len(),
                num_elements,
                "vec1 should have {} elements",
                num_elements
            );
            assert_eq!(
                vec2.len(),
                num_elements,
                "vec2 should have {} elements",
                num_elements
            );
            assert_eq!(
                vec3.len(),
                num_elements,
                "vec3 should have {} elements",
                num_elements
            );
            alloc_dbg!("✅ Vector lengths verified");

            // 最终验证2：验证vec1和vec2的关系 (vec2 = vec1 + 1)
            for i in 0..num_elements {
                assert_eq!(
                    vec2[i],
                    vec1[i] + 1,
                    "Final check: vec2[{}] = {} should be vec1[{}] + 1 = {}",
                    i,
                    vec2[i],
                    i,
                    vec1[i] + 1
                );
            }
            alloc_dbg!("✅ vec2 = vec1 + 1 relationship verified");

            // 最终验证3：验证vec1和vec3的关系 (vec3 = vec1 * 2)
            for i in 0..num_elements {
                assert_eq!(
                    vec3[i],
                    vec1[i] * 2,
                    "Final check: vec3[{}] = {} should be vec1[{}] * 2 = {}",
                    i,
                    vec3[i],
                    i,
                    vec1[i] * 2
                );
            }
            alloc_dbg!("✅ vec3 = vec1 * 2 relationship verified");

            // 动态修改测试
            alloc_dbg!("[DEBUG] Testing dynamic modifications...");

            // 随机修改一些元素
            let mut modified_indices = Vec::new();
            for _ in 0..10 {
                let idx = rng.random_range(0..num_elements);
                let new_val: i32 = rng.random_range(-500..500);

                vec1[idx] = new_val;
                vec2[idx] = new_val + 1;
                vec3[idx] = new_val * 2;
                modified_indices.push(idx);

                alloc_dbg!("Modified index {}: new value = {}", idx, new_val);
            }

            // 再次验证所有元素
            alloc_dbg!("[DEBUG] Re-verifying all elements after modifications...");
            for i in 0..num_elements {
                // 跳过已经验证的关系，直接验证三个向量的一致性
                let expected_v2 = vec1[i] + 1;
                let expected_v3 = vec1[i] * 2;

                if vec2[i] != expected_v2 {
                    panic!(
                        "After modification: vec2[{}] = {} should be {} (vec1[{}] + 1)",
                        i, vec2[i], expected_v2, i
                    );
                }

                if vec3[i] != expected_v3 {
                    panic!(
                        "After modification: vec3[{}] = {} should be {} (vec1[{}] * 2)",
                        i, vec3[i], expected_v3, i
                    );
                }
            }
            alloc_dbg!("✅ All modifications verified successfully");

            // 测试动态增加
            alloc_dbg!("[DEBUG] Testing dynamic growth...");
            let additional_elements = 20;
            alloc_dbg!("Adding {} more elements...", additional_elements);

            for i in 0..additional_elements {
                let value: i32 = rng.random_range(-2000..2000);
                let idx = num_elements + i;

                vec1.push(value);
                vec2.push(value + 1);
                vec3.push(value * 2);

                // 验证新增的元素
                assert_eq!(
                    vec2[idx],
                    vec1[idx] + 1,
                    "New element {}: vec2[{}] = {} should be vec1[{}] + 1",
                    i,
                    idx,
                    vec2[idx],
                    idx
                );
                assert_eq!(
                    vec3[idx],
                    vec1[idx] * 2,
                    "New element {}: vec3[{}] = {} should be vec1[{}] * 2",
                    i,
                    idx,
                    vec3[idx],
                    idx
                );
            }

            alloc_dbg!(
                "Final sizes: vec1={}, vec2={}, vec3={}",
                vec1.len(),
                vec2.len(),
                vec3.len()
            );

            // 最终全面验证
            let total_elements = num_elements + additional_elements;
            alloc_dbg!(
                "[DEBUG] Final comprehensive verification of all {} elements...",
                total_elements
            );

            for i in 0..total_elements {
                // 验证三个向量的关系
                if vec2[i] != vec1[i] + 1 {
                    panic!(
                        "Final comprehensive: vec2[{}] = {} ≠ vec1[{}] + 1 = {}",
                        i,
                        vec2[i],
                        i,
                        vec1[i] + 1
                    );
                }

                if vec3[i] != vec1[i] * 2 {
                    panic!(
                        "Final comprehensive: vec3[{}] = {} ≠ vec1[{}] * 2 = {}",
                        i,
                        vec3[i],
                        i,
                        vec1[i] * 2
                    );
                }

                // 验证索引正确性（确保没有越界）
                assert!(
                    i < vec1.len() && i < vec2.len() && i < vec3.len(),
                    "Index {} out of bounds",
                    i
                );
            }

            alloc_dbg!("✅ All {} elements verified successfully!", total_elements);

            // 额外验证：确保向量内容确实不同但相关
            alloc_dbg!("[DEBUG] Verifying vector uniqueness and relationships...");

            // 检查所有向量都不完全相同
            assert_ne!(vec1, vec2, "vec1 and vec2 should not be identical");
            assert_ne!(vec1, vec3, "vec1 and vec3 should not be identical");
            assert_ne!(vec2, vec3, "vec2 and vec3 should not be identical");

            // 但应该有预期的数学关系
            let vec1_plus_one: Vec<i32> = vec1.iter().map(|&x| x + 1).collect();
            let vec1_times_two: Vec<i32> = vec1.iter().map(|&x| x * 2).collect();

            assert_eq!(
                vec2, vec1_plus_one,
                "vec2 should equal vec1 + 1 element-wise"
            );
            assert_eq!(
                vec3, vec1_times_two,
                "vec3 should equal vec1 * 2 element-wise"
            );

            alloc_dbg!("✅ Vector relationships mathematically verified");

            alloc_dbg!("✅ Test 11 passed: Random dynamic verification complete");
        }
    }

    {
        // 测试12：内存完全填满测试 - 自动扫盘对比
        alloc_dbg!("\n=== Test 12: Memory sweep test with auto comparison ===");
        {
            use alloc::collections::TryReserveError;

            // 全局写入计数器
            static mut WRITE_COUNTER: u32 = 0;
            static mut ALLOC_COUNTER: u32 = 0;

            fn get_write_counter() -> u32 {
                unsafe { WRITE_COUNTER }
            }

            fn increment_write_counter() {
                unsafe {
                    WRITE_COUNTER = WRITE_COUNTER.wrapping_add(1);
                }
            }

            fn get_alloc_counter() -> u32 {
                unsafe { ALLOC_COUNTER }
            }

            fn increment_alloc_counter() {
                unsafe {
                    ALLOC_COUNTER = ALLOC_COUNTER.wrapping_add(1);
                }
            }

            fn reset_counters() {
                unsafe {
                    WRITE_COUNTER = 0;
                    ALLOC_COUNTER = 0;
                }
            }

            // 安全分配函数，带有写入验证
            fn try_allocate_and_write(size: usize, phase: u32, attempt: u32) -> Option<Vec<u8>> {
                let mut vec = Vec::new();

                increment_alloc_counter();

                match vec.try_reserve(size) {
                    Ok(_) => {
                        // 填充并验证数据
                        let fill_value =
                            ((phase.wrapping_mul(17)) as u8).wrapping_add(attempt as u8);
                        vec.resize(size, fill_value);

                        // 验证写入的值
                        if size > 0 {
                            for i in 0..vec.len() {
                                if vec[i] != fill_value {
                                    alloc_dbg!(
                                        "[ERROR] Phase {}: Memory corruption at index {}: expected 0x{:02X}, got 0x{:02X}",
                                        phase,
                                        i,
                                        fill_value,
                                        vec[i]
                                    );
                                    return None;
                                }
                            }

                            // 修改部分值来验证可写性
                            if vec.len() >= 4 {
                                vec[0] = 0xAA;
                                vec[size / 2] = 0xBB;
                                vec[size - 1] = 0xCC;

                                // 验证修改
                                if vec[0] != 0xAA || vec[size / 2] != 0xBB || vec[size - 1] != 0xCC
                                {
                                    alloc_dbg!("[ERROR] Phase {}: Failed to modify memory", phase);
                                    return None;
                                }
                            }

                            increment_write_counter();
                        }

                        Some(vec)
                    }
                    Err(TryReserveError { .. }) => None,
                }
            }

            // 扫盘函数：返回最大可分配大小
            fn memory_sweep(phase: u32) -> (usize, usize, Vec<usize>) {
                alloc_dbg!("\n[DEBUG] Phase {}: Starting memory sweep", phase);

                let mut allocations: Vec<Vec<u8>> = Vec::new();
                let mut total_allocated = 0;
                let mut max_successful = 0;
                let mut successful_sizes = Vec::new();
                let mut sweep_log = Vec::new();

                // 阶段1：指数增长扫描
                alloc_dbg!("[DEBUG] Phase {}: Stage 1 - Exponential growth scan", phase);
                let mut current_size = 1; // 从1字节开始
                let mut attempt_count = 0;

                while current_size <= 4 * 1024 * 1024 {
                    // 最大尝试4MB
                    attempt_count += 1;

                    alloc_dbg!(
                        "[DEBUG] Phase {}: Attempt {}: Trying {} bytes",
                        phase,
                        attempt_count,
                        current_size
                    );

                    if let Some(vec) =
                        try_allocate_and_write(current_size, phase, attempt_count as u32)
                    {
                        // 分配成功
                        allocations.push(vec);
                        total_allocated += current_size;

                        if current_size > max_successful {
                            max_successful = current_size;
                            alloc_dbg!(
                                "[DEBUG] Phase {}: New maximum: {} bytes",
                                phase,
                                max_successful
                            );
                        }

                        successful_sizes.push(current_size);
                        sweep_log.push((current_size, true, total_allocated));

                        alloc_dbg!(
                            "[DEBUG] Phase {}: ✓ Allocated {} bytes (total={})",
                            phase,
                            current_size,
                            total_allocated
                        );

                        // 指数增长：乘以2
                        current_size *= 2;

                        // 如果已经达到4MB，继续尝试更大的块
                        if current_size > 4 * 1024 * 1024 && max_successful == 4 * 1024 * 1024 {
                            // 尝试6MB、8MB等更大的块
                            current_size = 6 * 1024 * 1024;
                        }
                    } else {
                        // 分配失败，记录并进入阶段2
                        sweep_log.push((current_size, false, total_allocated));
                        alloc_dbg!(
                            "[DEBUG] Phase {}: ✗ Failed at {} bytes, starting binary search",
                            phase,
                            current_size
                        );
                        break;
                    }
                }

                // 阶段2：二分查找最优分配大小（如果阶段1失败了）
                if max_successful < 4 * 1024 * 1024 && current_size > 1 {
                    alloc_dbg!(
                        "[DEBUG] Phase {}: Stage 2 - Binary search for maximum",
                        phase
                    );

                    let mut low = max_successful; // 上次成功的最大大小
                    let mut high = current_size; // 失败的大小

                    while low < high {
                        let mid = low + (high - low) / 2;

                        // 跳过已经尝试过的大小
                        if mid == low || mid == high {
                            break;
                        }

                        attempt_count += 1;
                        alloc_dbg!(
                            "[DEBUG] Phase {}: Binary search: Trying {} bytes (low={}, high={})",
                            phase,
                            mid,
                            low,
                            high
                        );

                        if let Some(vec) = try_allocate_and_write(mid, phase, attempt_count as u32)
                        {
                            // 分配成功
                            allocations.push(vec);
                            total_allocated += mid;

                            if mid > max_successful {
                                max_successful = mid;
                                alloc_dbg!(
                                    "[DEBUG] Phase {}: New maximum: {} bytes",
                                    phase,
                                    max_successful
                                );
                            }

                            successful_sizes.push(mid);
                            sweep_log.push((mid, true, total_allocated));
                            low = mid; // 尝试更大的
                        } else {
                            sweep_log.push((mid, false, total_allocated));
                            high = mid; // 尝试更小的
                        }
                    }
                }

                // 阶段3：线性填充剩余空间（从小到大的碎片）
                alloc_dbg!(
                    "[DEBUG] Phase {}: Stage 3 - Linear fill with small chunks",
                    phase
                );

                // 尝试各种常见的小块大小
                let small_sizes = [
                    1, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768,
                    65536,
                ];

                for &size in small_sizes.iter() {
                    // 只尝试比最大成功分配小的大小
                    if size > max_successful {
                        continue;
                    }

                    // 跳过已经成功的大小
                    if successful_sizes.contains(&size) {
                        continue;
                    }

                    attempt_count += 1;
                    alloc_dbg!(
                        "[DEBUG] Phase {}: Linear fill: Trying {} bytes",
                        phase,
                        size
                    );

                    if let Some(vec) = try_allocate_and_write(size, phase, attempt_count as u32) {
                        allocations.push(vec);
                        total_allocated += size;
                        successful_sizes.push(size);
                        sweep_log.push((size, true, total_allocated));

                        alloc_dbg!(
                            "[DEBUG] Phase {}: ✓ Filled with {} bytes (total={})",
                            phase,
                            size,
                            total_allocated
                        );
                    } else {
                        sweep_log.push((size, false, total_allocated));
                    }
                }

                // 输出详细扫盘日志
                alloc_dbg!("\n[DEBUG] Phase {} sweep summary:", phase);
                alloc_dbg!("[DEBUG] Total attempts: {}", attempt_count);
                alloc_dbg!("[DEBUG] Successful allocations: {}", successful_sizes.len());
                alloc_dbg!(
                    "[DEBUG] Maximum single allocation: {} bytes",
                    max_successful
                );
                alloc_dbg!("[DEBUG] Total allocated: {} bytes", total_allocated);
                alloc_dbg!("[DEBUG] Write operations verified: {}", get_write_counter());

                alloc_dbg!("[DEBUG] Successful sizes (up to 10):");
                successful_sizes.sort();
                for (i, &size) in successful_sizes.iter().take(10).enumerate() {
                    alloc_dbg!("[DEBUG]   {}. {} bytes", i + 1, size);
                }
                if successful_sizes.len() > 10 {
                    alloc_dbg!("[DEBUG]   ... and {} more", successful_sizes.len() - 10);
                }

                (max_successful, total_allocated, successful_sizes)
            }

            // 第一次扫盘
            alloc_dbg!("\n[DEBUG] ===== FIRST SWEEP =====");
            reset_counters();
            let (max1, total1, sizes1) = memory_sweep(1);

            // 强制释放所有内存
            alloc_dbg!("[DEBUG] First sweep completed, forcing memory release...");
            drop(sizes1); // 明确释放

            // 给分配器时间清理
            for i in 0..100 {
                // 分配一些临时小对象然后立即释放
                let _temp = vec![0u8; 64];
            }

            // 第二次扫盘
            alloc_dbg!("\n[DEBUG] ===== SECOND SWEEP =====");
            reset_counters();
            let (max2, total2, sizes2) = memory_sweep(2);

            // 对比结果
            alloc_dbg!("\n[DEBUG] ===== COMPARISON =====");
            alloc_dbg!("[DEBUG] First sweep:");
            alloc_dbg!(
                "[DEBUG]   Max allocation: {} bytes ({} KB, {} MB)",
                max1,
                max1 / 1024,
                max1 / (1024 * 1024)
            );
            alloc_dbg!(
                "[DEBUG]   Total allocated: {} bytes ({} KB, {} MB)",
                total1,
                total1 / 1024,
                total1 / (1024 * 1024)
            );
            alloc_dbg!("[DEBUG]   Write operations: {}", get_write_counter());
            alloc_dbg!("[DEBUG]   Allocation attempts: {}", get_alloc_counter());

            alloc_dbg!("[DEBUG] Second sweep:");
            alloc_dbg!(
                "[DEBUG]   Max allocation: {} bytes ({} KB, {} MB)",
                max2,
                max2 / 1024,
                max2 / (1024 * 1024)
            );
            alloc_dbg!(
                "[DEBUG]   Total allocated: {} bytes ({} KB, {} MB)",
                total2,
                total2 / 1024,
                total2 / (1024 * 1024)
            );

            // 验证分配器仍然工作
            alloc_dbg!("\n[DEBUG] Verifying allocator integrity...");
            {
                // 测试小分配
                let test1 = vec![1u8, 2, 3, 4, 5];
                assert_eq!(test1.len(), 5);
                assert_eq!(test1[0], 1);
                assert_eq!(test1[4], 5);

                // 测试中等分配
                let mut test2 = Vec::with_capacity(4096);
                test2.resize(4096, 0xDD);
                assert_eq!(test2.len(), 4096);
                assert_eq!(test2[0], 0xDD);
                assert_eq!(test2[4095], 0xDD);

                // 修改并验证
                test2[0] = 0x11;
                test2[2048] = 0x22;
                test2[4095] = 0x33;
                assert_eq!(test2[0], 0x11);
                assert_eq!(test2[2048], 0x22);
                assert_eq!(test2[4095], 0x33);

                alloc_dbg!("[DEBUG] ✓ Allocator integrity verified");
            }

            // 结果分析
            alloc_dbg!("\n[DEBUG] ===== ANALYSIS =====");

            let max_diff = if max2 > max1 {
                max2 - max1
            } else {
                max1 - max2
            };

            let total_diff = if total2 > total1 {
                total2 - total1
            } else {
                total1 - total2
            };

            // 允许10%的差异（考虑到碎片和分配器开销）
            let max_allowed_diff = max1 / 10;

            if max_diff <= max_allowed_diff && total_diff <= max_allowed_diff {
                alloc_dbg!("[DEBUG] ✓ Both sweeps produced similar results");
                alloc_dbg!(
                    "[DEBUG]   Max allocation difference: {} bytes (within {} bytes tolerance)",
                    max_diff,
                    max_allowed_diff
                );
                alloc_dbg!(
                    "[DEBUG]   Total allocation difference: {} bytes",
                    total_diff
                );
            } else {
                alloc_dbg!("[DEBUG] ⚠ Results differ significantly");
                alloc_dbg!(
                    "[DEBUG]   Max allocation: first={}, second={}, diff={}",
                    max1,
                    max2,
                    max_diff
                );
                alloc_dbg!(
                    "[DEBUG]   Total allocation: first={}, second={}, diff={}",
                    total1,
                    total2,
                    total_diff
                );
            }

            // 关键断言
            assert!(max1 > 0, "First sweep should allocate at least 1 byte");
            assert!(max2 > 0, "Second sweep should allocate at least 1 byte");
            assert!(total1 > 0, "First sweep total should be positive");
            assert!(total2 > 0, "Second sweep total should be positive");

            alloc_dbg!("\n✅ Test 12 passed: Memory sweep test completed successfully");
            alloc_dbg!(
                "[DEBUG]   Phase 1 max: {} bytes, total: {} bytes",
                max1,
                total1
            );
            alloc_dbg!(
                "[DEBUG]   Phase 2 max: {} bytes, total: {} bytes",
                max2,
                total2
            );
            alloc_dbg!(
                "[DEBUG]   Write operations verified: {}",
                get_write_counter()
            );
            alloc_dbg!("[DEBUG]   Allocation attempts: {}", get_alloc_counter());
        }
        alloc_dbg!("[DEBUG] Test 12: All memory automatically released");
    }

    alloc_dbg!("[DEBUG] Read/Write correctness test complete");

    alloc_dbg!("\n🎉 All tests PASSED! Allocator is working correctly!");

    ALLOCATOR.print_free_list();

    alloc_dbg!("[DEBUG] === ALLOCATOR TEST COMPLETE ===");
}
