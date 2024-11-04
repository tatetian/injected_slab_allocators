// Version 1: An example implementation of naive slab caches of one-page capacity.

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

            static SINGLETON: SinglePageSlabCache<SLOT_SIZE, ()> = SinglePageSlabCache::new();

            fn recycle_slot(
                slot: FreeSlabSlot<SLOT_SIZE>,
                _extension: &dyn Any,
                _pin_cpu_guard: &dyn PinCurrentCpu,
            ) {
                SINGLETON.recycle_slot(slot);
            }

            SINGLETON.init(());
            &SINGLETON as &'static dyn AnySlabCache<SLOT_SIZE> 
        }
    }
}

pub struct SinglePageSlabCache<const SLOT_SIZE: usize, Ext> {
    slab: SpinLock<Option<Slab<SLOT_SIZE, Ext>>>,
}

impl<const SLOT_SIZE: usize, Ext> SinglePageSlabCache<SLOT_SIZE, Ext> {
    pub const fn new() -> Self {
        Self {
            slab: SpinLock::new(None),
        }
    }

    #[doc(hidden)]
    pub fn init(&self, recycle_slot_fn: RecycleSlotFn, slab_extension: Ext) {
        let mut slab_guard = self.slab.lock();
        let slab = Slab::alloc(recycle_slot_fn, slab_extension).unwrap();
        *slab_guard = Some(slab);
    }

    pub fn new_slot(&self) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        let mut slab_guad = self.slab.lock();
        let slab = slab_guard.as_mut().unwrap(); 
        slab.new_slot()
    }

    pub fn recycle_slot(&self, free_slot: FreeSlabSlot<SLOT_SIZE>) {
        let mut slab_guad = self.slab.lock();
        let slab = slab_guard.as_mut().unwrap(); 
        slab.recycle_slot(free_slot)
    }
}

impl<const SLOT_SIZE: usize> SlabSlotAlloc<SLOT_SIZE> for SinglePageSlabCache<SLOT_SIZE> {
    fn alloc(&self, _: &dyn PinCurrentCpu) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        self.new_slot()
    }
}
