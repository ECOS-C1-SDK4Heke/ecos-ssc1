//! 将：`RAM结束地址, _heap_start` 变为 allocator 分配的空间

#![allow(unused)]

use crate::println;

use buddy_system_allocator::LockedHeap;
use core::alloc::{GlobalAlloc, Layout};

unsafe extern "C" {
    static _heap_start: u8;
}

// 对于 8MB 内存，ORDER = 23 足够
const HEAP_ORDER: usize = 23;

pub struct GlobalAllocator {
    heap: LockedHeap<HEAP_ORDER>,
}

impl GlobalAllocator {
    pub const fn new() -> Self {
        Self {
            heap: LockedHeap::<HEAP_ORDER>::new(),
        }
    }

    pub unsafe fn init(&self) {
        unsafe {
            let heap_start = &_heap_start as *const u8 as usize;

            let ram_end: usize = 0x04800000;
            let heap_size = ram_end.saturating_sub(heap_start);

            if heap_size == 0 {
                panic!("No heap memory available!");
            }

            // 检查堆大小是否超过分配器支持的最大值
            let max_supported = 1usize << HEAP_ORDER;
            if heap_size > max_supported {
                panic!(
                    "Heap size {} bytes exceeds maximum supported {} bytes",
                    heap_size, max_supported
                );
            }

            self.heap.lock().init(heap_start, heap_size);

            println!(
                "Heap: 0x{:08X} - 0x{:08X} ({} bytes = {} MB)",
                heap_start,
                heap_start + heap_size,
                heap_size,
                heap_size / (1024 * 1024)
            );
        }
    }
}

unsafe impl GlobalAlloc for GlobalAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { self.heap.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.heap.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { self.heap.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { self.heap.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

pub unsafe fn init() {
    unsafe {
        GLOBAL_ALLOCATOR.init();
    }
}

/// 使用一些基础的动态变量测试分配器
pub fn test() {
    use alloc::string::String;
    use alloc::vec::Vec;

    println!("Testing allocator...");

    let mut v = Vec::new();
    for i in 0..10 {
        v.push(i);
    }
    println!("Vec: {:?}", v);

    let s = String::from("Hello allocator!");
    println!("String: {}", s);

    let b = alloc::boxed::Box::new(42);
    println!("Box: {}", *b);

    println!("Allocator test passed!");
}
