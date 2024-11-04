//! Version 3: An example implementation of lockless slab caches.
//!
//! This example implementation, `LocklessSlabCache`, shows how to
//! use CPU local storage to achieve lockess slab allocations and deallocations
//! for the common scanario when objects are allocated and deallocated
//! on the same CPU.

pub fn init() {
    let slab_cache_array = Box::new(SlabCacheArray {
        size16: new_static_slab_cache!(16),
        // ...
        size2048: new_static_slab_cache!(2048),
    }).leak();
    ostd::heap::inject_slab_cache_array(slab_cache_array);
}

macro_rules! new_static_slab_cache {
    ( $slot_size:expr ) => {
        {
            const SLOT_SIZE: usize = $slot_size;

            cpu_local! {
                static LOCAL_SLAB_CACHES: SinglePageSlabCache<SLOT_SIZE, SlabExt>= SinglePageSlabCache::new();
                static LOCAL_FREE_LIST: RefCell<FreeSlabSlotList<SLOT_SIZE>> = RefCell::new(None);
            }
            static SINGLETON: LocklessSlabCache = LocklessSlabCache::new(&LOCAL_SLAB_CACHES, &LOCAL_FREE_LIST);

            fn recycle_slot(
                slot: FreeSlabSlot<SLOT_SIZE>,
                extension: &dyn Any,
                pin_cpu_guard: &dyn PinCurrentCpu,
            ) {
                let owner_cpu = {
                    let extension = extension.downcast_ref::<SlabExt>().unwrap();
                    extension.owner_cpu
                };
                SINGLETON.recycle_slot(slot, owner_cpu, pin_cpu_guard);
            }

            SINGLETON.init();
            &SINGLETON as &'static dyn AnySlabCache<$slot_size> 
        }
    }
}

pub struct LocklessSlabCache<const SLOT_SIZE: usize> {
    local_slab_caches: &'static CpuLocal<SinglePageSlabCache<SLOT_SIZE>>,
    local_free_list: &'static CpuLocal<RefCell<FreeSlabSlotList>>,
}

struct SlabMeta {
    owner_cpu: CpuId,
}

impl<const SLOT_SIZE: usize> LocklessSlabCache<SLOT_SIZE> {
    pub const fn new(
        local_slab_caches: &'static CpuLocal<SinglePageSlabCache<SLOT_SIZE>>,
        local_free_list: &'static CpuLocal<RefCell<FreeSlabSlotList>>,
    ) -> Self {
        Self {
            local_slab_caches,
            local_free_list,
        }
    }

    fn init(&self) {
        for cpu_i in 0..cpu::num_cpus() {
            let local_slab_cache = self.local_slab_caches.get_on_cpu(cpu_i);
            let slab_extension = SlabExt {
                owner_cpu: cpu_i,
            };
            local_slab_cache.init(slab_extension, slot_recycle_fn);
        }
    }

    fn recycle_slot(
        &self,
        free_slot: FreeSlabSlot<SLOT_SIZE>,
        owner_cpu: CpuId,
        pin_cpu_guard: &dyn PinCurrentCpu,
    ) {
        // Fast path: the free slot belongs to the current CPU.
        if owner_cpu = pin_cpu_guard.current_cpu() {
            let free_list_cell = self.free_list.get_with(pin_cpu_guard);
            let free_list = free_list_cell.borrow_mut();
            free_list.push(slot);
            return;
        }
        
        // Slow path: returning the slot to the per-CPU slab cache
        // on the remote, owner CPU.
        let owner_slab_cache = self.local_slab_caches.get_on_cpu(owner_cpu);
        owner_slab_cache.recycle_slot(free_slot);
    }
}

impl<const SLOT_SIZE: usize> SlabSlotAlloc<SLOT_SIZE> for LocklessSlabCache<SLOT_SIZE> {
    fn alloc(&self, pin_cpu_guard: &dyn PinCurrentCpu) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        // Fast path: pop a free slot from the local free list
        let local_free_list_cell = self.free_list.get_with(pin_cpu_guard);
        let local_free_list = free_list_cell.borrow_mut();
        let free_slot = free_list.pop();
        if free_slot.is_some() {
            return free_slot;
        }

        // Slow path: try to get a free slot from the local, per-CPU slab ache
        let current_cpu = pin_cpu_guard.current_cpu();
        let local_slab_cache = self.local_slab_cache.get_on_cpu(current_cpu);
        local_slab_cache.new_slot()
    }
}

pub struct FreeSlabSlotList<const SLOT_SIZE: usize> {
    head: Option<FreeSlabSlot<SLOT_SIZE>>
}

impl<const SLOT_SIZE: usize> FreeSlabSlotList<SLOT_SIZE> {
    pub const fn new() -> Slef {
        Selef {
            head: None,
        }
    }

    pub fn push(&mut self, slot: FreeSlabSlot<SLOT_SIZE>) {
        todo!()
    }

    pub fn pop(&mut self) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        todo!()
    }
}