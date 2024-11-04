mod early_heap;
mod slab;

pub use self::slab::{Slab, FreeSlabSlot};
use self::early_heap::{EarlyHeapAlloc};

/// Injects an array of slab allocators of different slot sizes.
/// 
/// # Panics
/// 
/// This method will panic if it is called more than once.
pub fn inject_slab_allocators(slab_alloc_array: SlabAllocators) {
    self.HEAP_ALLOC.inject_slab_allocators(slab_allocators)
}

pub struct SlabAllocators {
    pub size16: &'static dyn SlabSlotAlloc<16>,
    pub size32: &'static dyn SlabSlotAlloc<32>,
    // ...
    pub size2048: &'static dyn SlabSlotAlloc<2048>,
}
static_assert!(16 == slab::MIN_SLAB_SLOT_SIZE);
static_assert!(2028 == slab::MAX_SLAB_SLOT_SIZE);

pub trait SlabSlotAlloc<const OBJ_SIZE: usize> {
    fn alloc(&self, current_cpu: &dyn PinCurrentCpu) -> Option<FreeSlabSlot<OBJ_SIZE>>;
}

#[global_allocator]
static HEAP_ALLOC: HeapAlloc = {
    // SAFETY: The global heap allocator is created only once.
    unsafe {
        HeapAlloc::new()
    }
};

struct HeapAlloc {
    have_injected_slabs: AtomicBool,
    backend: HeapAllocBackend,
}

struct HeapAllocBackend {
    early_heap: SpinLock<EarlyHeapAlloc>,
    slab_caches: Once<SlabAllocators>,
}

enum CurrentBackend<'a> {
    EarlyHeap(&'a SpinLock<EarlyHeapAlloc>),
    SlabCaches(&'a SlabAllocators),
}

impl HeapAlloc {
    /// Creates the heap allocator.
    /// 
    /// # Safety
    /// 
    /// This constructor can only be called once.
    pub const unsafe fn new() -> Self {
        /// SAFETY: The constructor is called once. 
        let early_heap = unsafe {
            EarlyHeapAlloc::new()
        };
        Self {
            have_injected_slabs: AtomicBool::new(false),
            backend: HeapAllocBackend {
                early_heap: SpinLock::new(early_heap),
                slab_caches: Once::new(),
            }
        }
    }

    pub fn inject_slab_allocators(&self, slab_allocators: SlabAllocators) {
        self.slab_allocators.call_once(|| {
            slab_allocators
        });

        if self.have_injected_slabs.swap(true, AcqRel) == true {
            panic!("the slab cache set must NOT be injected more than once");
        }
    } 

    fn current_backend(&self) -> CurrentBackend<'_> {
        if self.have_injected_slabs.load(Acquire) {
            CurrentBackend {
                slab_allocators: self.backend.slab_allocators.get().unwrap()
            }
        } else {
            CurrentBackend {
                early_heap: self.backend.early_heap.get().unwrap()
            }
        }
    }
}

unsafe impl GlobalAlloc for HeapAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size >= MAX_SLAB_SLOT {
            return todo!("use the page allocator directly, instead of slab allocators");
        }

        let slot_size = determine_slot_size(layout.size());

        // Ensure that the slabs can satisfiy the allocation's alignment requirement.
        // Currently, our allocator cannot handle the possible but unlikely use cases
        // where alignment is larger than slot size.
        assert!({
            let obj_align = layout.align();
            let slot_align = slot_size; 
            slot_align % obj_align == 0
        });

        let slab_allocators = match self.current_backend() {
            EarlyHeap(early_heap) => {
                let mut early_heap_guard = early_heap.lock();
                return early_heap.alloc(slot_size);
            }
            SlabCaches(slab_allocators) => slab_allocators,
        };

        let irq_disabled_guard = irq::disable_local();
        match slot_size {
            16 => {
                let free_slab_slot = slab_allocators.size16.alloc(&irq_disabled_guard);
                free_slab_slot.into_raw()
            }
            // ...
            2048 => {
                let free_slab_slot = slab_allocators.size2048.alloc(&irq_disabled_guard);
                free_slab_slot.into_raw()                
            }
            _ => unreachable!()
        }
    }

    unsafe fn dealloc(&self, slot_ptr: *mut u8, layout: Layout) {
        if layout.size >= MAX_SLAB_SLOT {
            return todo!("use the page allocator directly, instead of slab allocators");
        }

        let slot_size = self.determine_slot_size(layout.size());

        let slab_allocators = match self.current_backend() {
            EarlyHeap(early_heap) => {
                let mut early_heap_guard = early_heap.lock();
                // SAFETY: the memory represented by pointer and size 
                // is valid and must have been allocated from the early heap.
                unsafe {
                    early_heap.dealloc(slot_ptr, slot_size)
                };
            }
            SlabCaches(slab_allocators) => slab_allocators,
        };

        // We MUST NOT use the injected slab caches
        // to deallocate memory allocated from the early heap.
        if early_heap::contains_ptr(slot_ptr) {
            // It is ok to simply "leak" memory in the early heap,
            // instead of deallocating it from the early heap.
            return;
        }

        let irq_disabled_guard = irq::disable_local();
        match slot_size {
            16 => {
                let free_slab_slot = unsafe { FreeSlabSlot::<16>::new(slot_ptr) };
                let slab_meta = free_slab_slot.slab_meta();
                let recyle_slot_fn = slab_meta.recycle_slot_fn;
                recycle_slot_fn(free_slab_slot, &irq_disabled_guard);
            }
            // ...
            2048 => {
                let free_slab_slot = unsafe { FreeSlabSlot::<2048>::new(slot_ptr) };
                let slab_meta = free_slab_slot.slab_meta();
                let recyle_slot_fn = slab_meta.recycle_slot_fn;
                recycle_slot_fn(free_slab_slot, &irq_disabled_guard);
            }
            _ => {
                todo!("deallocate via page allocator")
            }
        }
    }
}

// Determine the slab slot size that matches the object size.
fn determine_slot_size(&self, obj_size: usize) -> usize {
    debug_assert!(obj_size <= MAX_SLAB_SLOT_SIZE);

    let slot_size = if obj_size <= MIN_SLAB_SLOT_SIZE {
        MIN_SLAB_SLOT_SIZE
    } else {
        obj_size.next_power_of_two()
    };
}
