use std::alloc::{GlobalAlloc, Layout};
use windows_sys::Win32::System::Memory::{
    GetProcessHeap, HEAP_ZERO_MEMORY, HeapAlloc, HeapFree, HeapReAlloc,
};

pub struct ProcessHeapAlloc;

unsafe impl GlobalAlloc for ProcessHeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        HeapAlloc(GetProcessHeap(), 0, layout.size()) as *mut u8
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, layout.size()) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        if !ptr.is_null() {
            HeapFree(GetProcessHeap(), 0, ptr.cast());
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, _layout: Layout, new_size: usize) -> *mut u8 {
        HeapReAlloc(GetProcessHeap(), 0, ptr.cast(), new_size) as *mut u8
    }
}
