use component_sparse::ComponentSparse;
use std::{
    alloc::Layout,
    collections::HashMap,
    ptr::{NonNull, swap_nonoverlapping},
    rc::Rc,
};
use table_index::TableId;

use crate::{
    component::ComponentLocation,
    entity::Entity,
    pointer::{Ptr, PtrMut},
    type_info::TypeInfo,
};

pub mod component_sparse;
pub mod table;
pub mod table_data;
pub mod table_index;

/// Trait for allocating and reallocating memory for a type-erased array.
///
/// Currently implemented on [Column] and [NonNull<Entity>].
trait TypeErased {
    /// Grow the array to the new capacity.
    ///
    /// # Safety
    /// - The caller must ensure that the array currently does not have any allocation.
    unsafe fn alloc(&mut self, new_cap: usize);

    /// Grow the array to the new capacity.
    ///
    /// # Safety
    /// - The caller must ensure that the new capacity is greater than the current capacity.
    /// - The caller must ensure that `old_cap` is the current capacity of the array.
    unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize);
}

pub(crate) struct Column {
    /// Component [Entity] that owns this column.
    ///
    /// This may be different from the entity on type_info.
    component: Entity,
    pub(super) data: NonNull<u8>,
    pub(super) type_info: Rc<TypeInfo>,
}

impl Column {
    pub fn new(component: Entity, type_info: Rc<TypeInfo>) -> Self {
        Self {
            component,
            data: NonNull::dangling(),
            type_info,
        }
    }

    #[inline]
    pub(crate) fn id(&self) -> Entity {
        self.component
    }

    /// #Safety
    /// Caller must ensure that `row` is valid for this column.
    #[inline]
    unsafe fn get(&self, row: usize) -> Ptr {
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { Ptr::new(self.data.add(row * self.type_info.size())) }
    }

    /// #Safety
    /// Caller must ensure that `row` is in bounds for column.
    #[inline]
    unsafe fn get_mut(&self, row: usize) -> PtrMut {
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { PtrMut::new(self.data.add(row * self.type_info.size())) }
    }

    /// Swap rows `a` and `b`.
    ///
    /// This function does not do any bounds checking.
    ///
    /// # Safety
    /// - Caller must ensure that `a` and `b` are in bounds for column.
    /// - Caller must ensure that `a` and `b` are not equal.
    #[inline]
    unsafe fn swap_nonoverlapping(&self, a: usize, b: usize) {
        debug_assert!(a != b, "tried to swap with itself");

        // SAFETY:
        // data is non-null
        // caller guarantees a and b are valid and not equal.
        unsafe {
            let size = self.type_info.size();
            let base = self.data.as_ptr();
            let a_ptr = base.add(a * size);
            let b_ptr = base.add(b * size);
            std::ptr::swap_nonoverlapping(a_ptr, b_ptr, size);
        }
    }
}

impl TypeErased for Column {
    unsafe fn alloc(&mut self, new_cap: usize) {
        let (size, align) = self.type_info.size_align();
        let new_layout = Layout::from_size_align(new_cap * size, align).expect("Invalid layout");
        let new_ptr = unsafe { std::alloc::alloc(new_layout) };

        self.data = match NonNull::new(new_ptr) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout),
        };
    }

    unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        debug_assert!(new_cap > old_cap, "tried to realloc with smaller capacity");

        let (size, align) = self.type_info.size_align();
        let new_layout = Layout::from_size_align(new_cap * size, align).expect("Invalid layout");
        let old_layout = Layout::from_size_align(old_cap * size, align).expect("Invalid layout");
        let new_ptr =
            unsafe { std::alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size()) };

        self.data = match NonNull::new(new_ptr) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout),
        };
    }
}

impl TypeErased for NonNull<Entity> {
    unsafe fn alloc(&mut self, new_cap: usize) {
        let new_layout = Layout::array::<Entity>(new_cap).expect("Invalid laout");
        let new_ptr = unsafe { std::alloc::alloc(new_layout) };

        *self = match NonNull::new(new_ptr as *mut Entity) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout),
        };
    }

    unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        debug_assert!(new_cap > old_cap, "realloc with smaller capacity");

        let new_layout = Layout::array::<Entity>(new_cap).expect("Invalid layout");
        let old_layout = Layout::array::<Entity>(old_cap).expect("Invalid layout");
        let old_ptr = self.as_ptr() as *mut u8;
        let new_ptr = unsafe { std::alloc::realloc(old_ptr, old_layout, new_layout.size()) };

        *self = match NonNull::new(new_ptr as *mut Entity) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout),
        };
    }
}

/// Swaps rows `a` and `b`
///
/// This function does not perform any bounds checking.
///
/// # Safety
/// - The caller must ensure that `a` and `b` are valid for this array.
/// - The caller must ensure that `a` and `b` are different rows.
unsafe fn swap_entities(entities: &mut NonNull<Entity>, a: usize, b: usize) {
    debug_assert!(a != b, "attempting to swap same memory location");

    // SAFETY:
    // - The caller must ensure that `row` and `last` are valid rows.
    // - row and last are guaranteed not to overlap, since they are different rows.
    unsafe {
        let base = entities.as_ptr();
        let ap = base.add(a);
        let bp = base.add(b);
        std::ptr::swap_nonoverlapping(ap, bp, 1);
    }
}

pub enum StorageType {
    Tables,
    Sparse,
}

pub enum Storage {
    Sparse(ComponentSparse),
    Tables(HashMap<TableId, ComponentLocation>),
}
