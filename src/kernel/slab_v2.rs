//! Version 2: An example implementation of per-CPU slab caches.
//!
//! This example implementation, `ScalableSlabCache`, shows two useful techniques:
//! 1. Creating per-CPU slab caches to reduce lock contention.
//! 2. Making use of the custom metadata associated with a `Slab`.

pub fn init() {
    let slab_cache_array = Box::new(SlabAllocators {
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
                static LOCAL_SLAB_CACHES: SinglePageSlabCache<SLOT_SIZE, SlabExt> = SinglePageSlabCache::new();
            }
            static SINGLETON: ScalableSlabCache<SLOT_SIZE> = ScalableSlabCache::new(&LOCAL_SLAB_CACHES);

            fn recycle_slot(
                slot: FreeSlabSlot<SLOT_SIZE>,
                extension: &dyn Any,
                _pin_cpu_guard: &dyn PinCurrentCpu,
            ) {
                let extension = extension.downcast_ref::<SlabExt>().unwrap();
                SINGLETON.recycle_slot(slot, extension.owner_cpu);
            }

            SINGLETON.init();
            &SINGLETON as &'static dyn AnySlabCache<SLOT_SIZE> 
        }
    }
}

pub struct ScalableSlabCache<const SLOT_SIZE: usize> {
    local_slab_caches: &'static CpuLocal<SinglePageSlabCache<SLOT_SIZE, SlabExt>>,
}

struct SlabExt {
    owner_cpu: CpuId,
}

impl<const SLOT_SIZE: usize> ScalableSlabCache<SLOT_SIZE> {
    pub const fn new(
        local_slab_caches: &'static CpuLocal<SinglePageSlabCache<SLOT_SIZE, SlabExt>>,
    ) -> Self {
        Self {
            local_slab_caches,
        }
    }

    #[doc(hidden)]
    pub fn init(&self, slot_recycle_fn: SlotRecycleFn) {
        for cpu_i in 0..cpu::num_cpus() {
            let local_slab_cache = self.local_slab_caches.get_on_cpu(cpu_i);
            let slab_extension = SlabExt {
                owner_cpu: cpu_i,
            };
            local_slab_cache.init(slab_extension, slot_recycle_fn);
        }
    }

    fn recycle_slot(&self, free_slot: FreeSlabSlot<SLOT_SIZE>, owner_cpu: CpuId) {
        let owner_slab_cache = self.per_cpu.get_on_cpu(owner_cpu);
        owner_slab_cache.recycle_slot(free_slot);
    }
}

impl<const SLOT_SIZE: usize> SlabSlotAlloc<SLOT_SIZE> for ScalableSlabCache<SLOT_SIZE> {
    fn alloc(&self, pin_cpu_guard: &dyn PinCurrentCpu) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        let current_cpu = pin_cpu_guard.current_cpu();
        let local_slab_cache = self.per_cpu.get_on_cpu(current_cpu);
        local_slab_cache.new_slot()
    }
}
