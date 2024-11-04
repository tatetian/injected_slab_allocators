//! A slab is one or multiple contiguous pages
//! that are divided into a number of fixed-sized slots,
//! each of which can be used as the storage for an object
//! whose size is no greater than the slot size.
pub struct Slab<const SLOT_SIZE: usize, Ext> {
    page: NonNull<u8>,
}

impl<const SLOT_SIZE: usize, Ext> Slab<SLOT_SIZE, Ext> {
    /// Allocate a page-sized slab with the user-given slab extension.
    pub fn alloc(
        slot_recyle_fn: SlotRecycleFn<SLOT_SIZE>,
        extension: Ext
    ) -> Option<Self> {
        todo!("
            Step 1. Allocate a new page for slab with the specified metadata and extension.
            Step 2. Partition the slab as an array of FreeSlabSlot.
            Step 3. Link all FreeSlabSlots into a list.
        ")
    }

    pub fn new_slot(&mut self) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        let slab_meta = self.slab_meta();

        let head_ptr = slab_meta.free_list.load(Relaxed);
        if head_ptr == ptr::null() {
            return None();
        }

        let new_head_ptr = {
            let head = unsafe { &*head_ptr };
            head.next
        };
        slab_meta.free_list.store(new_head_ptr, Relaxed);

        // SAFETY: The pointer refers to a valid and unused free slot
        let new_slab_slot = unsafe {
            FreeSlabSlot::new(head_ptr as _)
        };

        slab_meta.nr_inuse_slots.fetch_add(1, Relaxed);

        Some(new_slab_slot)
    }

    pub fn recycle_slot(&mut self, free_slot: FreeSlabSlot<SLOT_SIZE>) {
        let slab_meta = self.slab_meta();

        // Safety invariant: a free slot is always returned to its parent slab.
        assert!({
            let expected_meta_ptr = slab_meta as *const SlabMeta;
            let actual_meta_ptr == free_slot.slab_meta() as _;
            actual_meta_ptr == expected_meta_ptr
        });

        let old_head_ptr = slab_meta.free_list.load(Relaxed);

        let new_head_ptr = {
            let linked_slot_ptr = free_slot.as_ptr() as *mut LinkedSlabSlot;
            // Avoid pointer aliasing due to the coexistence of FreeSlabSlot and LinkedSlabSlot.
            drop(free_slot);
            linked_slot_ptr
        };
        let new_head = unsafe {
            &mut *new_head_ptr
        };
        new_head.next = old_head_ptr;

        let old_count = slab_meta.nr_inuse_slots.fetch_sub(1, Relaxed);
        debug_assert!(old_count >= 1);
    }

    pub const fn nr_total_slots(&self) -> usize {
        PAGE_SIZE / SLOT_SIZE 
    }

    pub fn has_unused_slots(&self) -> bool {
        let slab_meta = self.slab_meta();
        slab_meta.free_list.load(Relaxed) != ptr::null()
    }

    pub fn nr_used_slots(&self) -> usize {
        slab_meta.nr_used_slots.load(Relaxed) as _
    }

    pub(crate) fn slab_meta(&self) -> &SlabMeta<Ext> {
        &self.slab_meta()
    }

    pub fn slab_extension(&self) -> &Ext {
        &self.slab_meta().extension
    }
}

impl<const SLOT_SIZE: usize, Ext> Drop for Slab<SLOT_SIZE, Ext> {
    fn drop(&mut self) {
        assert!(self.nr_used_slots() == 0);
    }
}

const fn does_slot_size_match_obj_size(real_slot_size: usize, obj_size: usize) {
    let expected_slot_size = super::determine_slot_size(obj_size);
    real_slot_size == expected_slot_size
}

pub struct FreeSlabSlot<const SLOT_SIZE: usize> {
    ptr: NonNull<[u8; SLOT_SIZE]>,
}

impl<const SLOT_SIZE: usize> FreeSlabSlot<SLOT_SIZE> {
    pub const ALIGN_SIZE: usize = SLOT_SIZE;

    pub unsafe fn new(ptr: *mut u8) -> Self {
        static_assert!(SLOT_SIZE.is_power_of_two());
        debug_assert!((ptr as usize) % Self::ALIGN_SIZE == 0);

        Self {
            ptr: NonNull::new_unchecked(ptr)
        }
    }

    pub fn into_raw(self) -> *mut u8 {
        todo!()        
    }

    pub unsafe fn from_raw(raw: *mut u8) -> Self {
        todo!()        
    }

    pub fn into_box<T>(self, obj: T) -> Box<T> {
        static_assert!(does_slot_size_match_obj_size(SLOT_SIZE, mem::size_of::<T>()));
        static_assert!(SLOT_SIZE % mem::align_of::<T>() == 0);

        let obj_ptr = self.ptr.as_mut_ptr() as *mut T;
        // SAFETY: 
        // 1. The slot is free;
        // 2. The size and alignment of the slot satisfies all the 
        // requirements by `T`.
        unsafe {
            obj_ptr.write(obj);
        }
    }

    pub fn from_box<T>(boxed_obj: Box<T>) -> Self {
        static_assert!(does_slot_size_match_obj_size(SLOT_SIZE, mem::size_of::<T>()));

        let obj_ptr = Box::leak(boxed_obj) as *mut T;
        // SAFETY: The pointer refer to a valid object.
        // And after the in-place drop, the object will not be used any more.
        unsafe {
            ptr::drop_in_place(obj_ptr);
        }

        let slot_ptr = obj_ptr as *mut u8;
        // SAFETY: Every object of Box<T> corresponds to an object of 
        // FreeSlabSlot<SLOT_SIZE>. The orignal object of Box<T> is no longer
        // used; so it can be converted to the corresponding FreeSlabSlot. 
        unsafe {
            Self::new(slot_ptr)
        }
    }

    pub fn into_arc<T>(self, data: T) -> Arc<T> {
        static_assert!(does_slot_size_match_obj_size(SLOT_SIZE, mem::size_of::<ArcInner<T>>()));
        static_assert!(SLOT_SIZE % mem::align_of::<ArcInner<T>>() == 0);

        // Same as what `Arc::new` does.
        let inner = ArcInner {
            strong: atomic::AtomicUsize::new(1),
            weak: atomic::AtomicUsize::new(1),
            data,
        };
        let ptr = Box::leak(self.into_box(inner)) as *mut ArcInner<T>;
        // SAFETY: The data structure of `Arc<T>` is defined as 
        //
        // pub struct Arc<T: ?Sized> {
        //     ptr: NonNull<ArcInner<T>>,
        //     phantom: PhantomData<ArcInner<T>>,
        // }
        //
        // and `ptr` is not null. So this transmute is correct. 
        unsafe {
            mem::transmute(ptr)
        }
    }

    pub fn from_arc<T>(arc: Arc<T>) -> Self {
        static_assert!(does_slot_size_match_obj_size(SLOT_SIZE, mem::size_of::<ArcInner<T>>()));

        todo!()
    }

    pub fn take_next_slot(&mut self) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        todo!()
    }

    pub fn replace_next_slot(&mut self, new: FreeSlabSlot<SLOT_SIZE>) -> Option<FreeSlabSlot<SLOT_SIZE>> {
        todo!()
    }

    pub fn next_slot(&self) -> Option<&FreeSlabSlot<SLOT_SIZE>> {
        todo!()
    }

    fn slab_meta(&self) -> &SlabMeta<()> {
        todo!()
    }
}

impl<const SLOT_SIZE: usize, Ext> Drop for FreeSlabSlot<SLOT_SIZE, Ext> {
    fn drop(&mut self) {
        // The parent slab can only be droppped if this counter is reduced to zero
        self.slab_meta().nr_inuse_slots.fetch_sub(1, Release);
    }
}

// The inner data structure of Arc<T> on the heap.
//
// The interface of `Arc<T, A>` already allows
// specifying a custom allocator of type `A: Allocator`.
// So in theory, allocating an `Arc` from `FreeSlabSlot`
// can be expressed as `Arc::<T, FreeSlabSlot>::new()`
// if we implement the `Allocator` trait for `FreeSlabSlot`.
// However, this is undesirable because `FreeSlabSlot` is not zero-sized,
// making `Arc<T, FreeSlabSlot>` even fatter than regular fat pointers.
// 
// Thus, we have no choice but to extract some knowledge about
// this `ArcInner<T>` from the alloc crate and embed it in our code.
// This allows us to manually construct `Arc` using the space of `FreeSlabSlot`.
// Although this implementation is kind of "ugly",
// it is still maintainable as `Arc` is quite fundamental to Rust
// and rather stable. Thus, our hardcoded knowledge is unlike to go invalid.
//
// Ref: https://doc.rust-lang.org/src/alloc/sync.rs.html
#[repr(C)]
struct ArcInner<T: ?Sized> {
    strong: atomic::AtomicUsize,
    weak: atomic::AtomicUsize,
    data: T,
}

/// The metadata for a slab.
// It is important to specify `repr(c)` here,
// which ensures that the memory layout of `SlabMeta<SLOT_SIZE, Ext>` and 
// `SlabMeta<SLOT_SIZE, ()>` are the same except for the last field.
#[repr(C)]
pub(crate) struct SlabMeta<const SLOT_SIZE: usize, Ext> {
    free_list: AtomicPtr<LinkedSlabSlot>,
    nr_inuse_slots: AtomicU16,
    slot_recyle_fn: SlotRecycleFn,
    // The extension provided by the OSTD user is stored in two fields.
    //
    // The first field stores the vtable of `Ext`` as an `dyn Any` trait object.
    extension_vtable: DynMetadata<dyn Any + 'static>,
    // The second first stores the real content of `Ext`.
    extension: Ext,
}

impl<const SLOT_SIZE: usize, Ext> SlabMeta<SLOT_SIZE, Ext> {
    pub fn extension(&self) -> &Ext {
        &self.extension
    }

    /// Gets the extension as an `Any` .
    /// 
    /// Use this method in cases when the concrete type of `Ext` has been erased.
    /// For example, the `SlotRecycleFn` function signatures
    /// takes the type-erased slab metadata of `SlabMeta<_, ()>`.
    pub fn extension_as_any(&self) -> &dyn Any {
        todo!("recover the trait object from user_meta_vtable") 
    }
}

pub type SlotRecycleFn<const SLOT_SIZE: usize> = fn(
    /* slot: */FreeSlabSlot<SLOT_SIZE>,
    /* extension: */&dyn Any,
    /* pin_cpu_guard: */&dyn PinCurrentCpu,
);