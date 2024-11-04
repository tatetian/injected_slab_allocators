/// A heap allocator for the early heap.
pub struct EarlyHeapAlloc {
    free_list_16: *mut LinkedFreeSlot,
    // ...
    free_list_2048: *mut LinkedFreeSlot,
}

impl EarlyHeapAlloc {
    /// Create a heap allocator.
    /// 
    /// # Safety
    /// 
    /// The early heap allocator must be a singleton as
    /// the early heap is a global memory region allocated statically.
    pub const unsafe fn new() -> Self {
        Self {
            free_list_16: ptr::null(),
            // ..
            free_list_2048: ptr::null(),
        }
    }

    pub fn alloc(&mut self, slot_size: usize) -> *mut u8 {
        match slot_size {
            16 => {
                todo!("try to reuse the existing slots in the free list;
                    if it is empty, allocate more free slots from the early heap pages.")
            }
            // ..
            2048 => {
                // Same as above
            }
            _ => unreachable!("slot size must be a valid slot size"),
        }
    }

    pub unsafe fn dealloc(&mut self, slot_ptr: *mut u8, slot_size: usize) -> *mut u8 {
        match slot_size {
            16 => {
                todo!("insert the slot back into the free list")
            }
            // ..
            2048 => {
                // Same as above
            }
            _ => unreachable!("slot size must be a valid slot size"),
        }
    }
}

struct LinkedFreeSlot {
    next: *mut LinkedFreeSlot,
}

/// Returns whether a pointer belongs to the early heap.
pub fn contains_ptr(ptr: *mut u8) -> bool {
    let heap_page_start = &EARLY_HEAP_PAGES.0 as usize;
    let heap_page_end = heap_page_start + NR_EARLY_HEAP_PAEGS * PAGE_SIZE;
    let ptr_addr = ptr as usize;
    heap_page_start <= ptr_addr && ptr_addr < heap_page_end
}

// The static memory region for the early heap.

const NR_EARLY_HEAP_PAEGS: usize = 256;    

#[repr(align(4096))]
struct EarlyHeapPages([[u8; PAGE_SIZE]; NR_EARLY_HEAP_PAEGS]);

impl EarlyHeapPages {
    pub fn new() -> Self {
        todo!()
    }
}

static mut EARLY_HEAP_PAGES: EarlyHeapPages = EarlyHeapPages::new();

static NR_USED_PAGES: AtomicU16 = AtomicU16::new();