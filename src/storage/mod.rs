pub mod archetype;
pub mod archetype_index;

use std::{alloc::Layout, ptr::NonNull, rc::Rc};
use crate::{entity::Entity, entity_flags::EntityFlags, entity_index::EntityIndex, id::Id, pointer::{Ptr, PtrMut}, type_info::TypeInfo};


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
    /// Component Id in archetype that owns this column.
    /// 
    /// This id may be different from the id on type_info.
    component: Id,
    pub(super) data: NonNull<u8>,
    pub(super) type_info: Rc<TypeInfo>,
}

impl Column {
    pub fn new(component: Id, type_info: Rc<TypeInfo>) -> Self {
        Self {
            component,
            data: NonNull::dangling(),
            type_info,
        }
    }

    #[inline]
    pub(crate) fn id(&self) -> Id {
        self.component
    }

    /// #Safety
    /// Caller must ensure that `row` and `size` are valid for this column.
    #[inline]
    unsafe fn get(&self, row: usize, size: usize) -> Ptr {
        // SAFETY:
        // data is non-null
        // caller guarantees row and size are valid.
        unsafe { Ptr::new(self.data.add(row * size)) }
    }

    /// #Safety
    /// Caller must ensure that `row` and `size` are valid for this column.
    #[inline]
    unsafe fn get_mut(&self, row: usize, size: usize) -> PtrMut {
        // SAFETY:
        // data is non-null
        // caller guarantees row and size are valid.
        unsafe { PtrMut::new(self.data.add(row * size)) }
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
        debug_assert!(new_cap > old_cap, "realloc with smaller capacity");

        let (size, align) = self.type_info.size_align();
        let new_layout = Layout::from_size_align(new_cap * size, align).expect("Invalid layout");
        let old_layout = Layout::from_size_align(old_cap * size, align).expect("Invalid layout");
        let new_ptr = unsafe { std::alloc::realloc(self.data.as_ptr(), old_layout, new_layout.size()) };

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
            None => std::alloc::handle_alloc_error(new_layout)
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
            None => std::alloc::handle_alloc_error(old_layout)
        };
    }
}

impl TypeErased for NonNull<EntityFlags> {
    unsafe fn alloc(&mut self, new_cap: usize) {
        let new_layout = Layout::array::<EntityFlags>(new_cap).expect("Invalid laout");
        let new_ptr = unsafe { std::alloc::alloc(new_layout) };

        *self = match NonNull::new(new_ptr as *mut EntityFlags) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout)
        };
    }

    unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        debug_assert!(new_cap > old_cap, "realloc with smaller capacity");
        
        let new_layout = Layout::array::<EntityFlags>(new_cap).expect("Invalid layout");
        let old_layout = Layout::array::<EntityFlags>(old_cap).expect("Invalid layout");
        let old_ptr = self.as_ptr() as *mut u8;
        let new_ptr = unsafe { std::alloc::realloc(old_ptr, old_layout, new_layout.size()) };
        
        *self = match NonNull::new(new_ptr as *mut EntityFlags) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(old_layout)
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

pub(crate) struct ArchetypeData {
    pub(super) entities: NonNull<Entity>,
    pub(super) flags: NonNull<EntityFlags>,
    pub(super) columns: Box<[Column]>,
    len: usize,
    cap: usize,
}

impl ArchetypeData {
    pub fn new(columns: Box<[Column]>) -> Self {
        Self {
            entities: NonNull::dangling(),
            flags: NonNull::dangling(),
            columns,
            len: 0,
            cap: 0,
        }
    }
    
    /// Returns number of rows in this archetype.
    #[inline]
    pub fn count(&self) -> usize {
        self.len
    }

    fn reserve(&mut self, additional: usize) {
        let required_cap = self.len.checked_add(additional).expect("capacity overflow");

        if required_cap <= self.cap {
            return;
        }

        unsafe {
            if self.cap == 0 {
                self.entities.alloc(required_cap);
                self.flags.alloc(required_cap);
                self.columns.iter_mut().for_each(|col| col.alloc(required_cap));
            }
            else {
                self.entities.realloc(self.cap, required_cap);
                self.flags.realloc(self.cap, required_cap); 
                self.columns.iter_mut().for_each(|col| col.realloc(self.cap, required_cap));
            };
        }
        
        self.cap = required_cap;
    }

    /// Creates a new row without initializing its elements.
    /// This function will grow all columns if necessary.
    /// 
    /// # Safety
    /// - The rows for the new entity in all columns will be uninitialized (hence, unsafe).
    /// - The caller must ensure to initialize the new row in all columns before using it.
    pub unsafe fn new_row_uninit(&mut self, entity: Entity) -> usize {
        // TODO: check if I should use `[Self::grow]` instead
        if self.len == self.cap { self.reserve(1); } 
    
        // SAFETY: 
        // * Pointer offset properly calculated.
        // * NonNull ptr safe to write.
        unsafe { 
            self.entities.as_ptr().add(self.len).write(entity);
            self.flags.as_ptr().add(self.len).write(EntityFlags::empty());
        }

        let row = self.len; self.len += 1;
        row
    }

    /// Returns a [Ptr] to the element at `row`, in `column`, without doing bounds checking.
    /// 
    /// # Safety
    /// - The caller ensures that `column` is valid.
    /// - The caller ensures that `row` is valid.
    pub unsafe fn get_unchecked(&self, column: usize, row: usize) -> Ptr {
        debug_assert!(column < self.columns.len(), "column out of bounds");
        debug_assert!(row < self.len, "row out of bounds");

        // SAFETY: The caller ensures that `row` and `column` are valid.
        unsafe {
            let col = self.columns.get_unchecked(column);
            col.get(row, col.type_info.size())
        }
    }

    /// Returns a [PtrMut] to the element at `row`, in `column`, without doing bounds checking.
    /// 
    /// # Safety
    /// - The caller ensures that `column` is in bounds.
    /// - The caller ensures that `row` is in bounds.
    pub unsafe fn get_unchecked_mut(&mut self, column: usize, row: usize) -> PtrMut {
        debug_assert!(column < self.columns.len(), "column out of bounds");
        debug_assert!(row < self.len, "row out of bounds");

        // SAFETY: The caller ensures that `row` and `column` in bounds.
        unsafe {
            let col = self.columns.get_unchecked_mut(column);
            col.get_mut(row, col.type_info.size())
        }
    }

    /// Returns the entity at the specified `row`.
    /// 
    /// # Safety
    /// - The row must be in-bounds (`row` < `self.len`).
    pub unsafe fn get_entity_unchecked(&self, row: usize) -> Entity {
        debug_assert!(row < self.len, "row out of bounds");
        // SAFETY: The caller ensures that `row` is valid.
        unsafe { *(self.entities.as_ptr().add(row)) }
    }

    /// Returns the entity flags at the specified `row`.
    /// 
    /// # Safety
    /// - The row must be in-bounds (`row` < `self.len`).
    pub unsafe fn get_flags_mut(&self, row: usize) -> &mut EntityFlags {
        debug_assert!(row < self.len, "row out of bounds");
        // SAFETY: The caller ensures that `row` is valid.
        unsafe { self.flags.add(row).as_mut() }
    }

    /// Deletes the row by swapping with the last row 
    /// and returns the entity that was in the last row 
    /// or `None` if `row` was the last.
    pub(super) fn delete_row(&mut self, entity_index: &mut EntityIndex, row: usize, should_drop: Vec<bool>) {
        debug_assert!(row < self.len, "row out of bounds");
        debug_assert!(self.columns.len() == should_drop.len());

        let last = self.len - 1;
        
        unsafe {
            // Check is done outside loop to avoid doing the same check for all columns.
            if row != last {
                swap_entities(&mut self.entities, row, last);

                // Drop the values in row, then move values from last row into row.
                for (i, col) in self.columns.iter().enumerate() {
                    let ti = &col.type_info;
                    let size = ti.size();
                    let row_ptr = col.data.add(row * size);
                    let last_ptr = col.data.add(last * size);

                    match ti.hooks.drop_fn {
                        Some(drop) if should_drop[i] => drop(row_ptr),
                        _ => {},
                    }

                    (ti.hooks.move_fn)(last_ptr, row_ptr)
                }

                // Update entity record. 
                // Allowed to panic since last row must contain a valid entity.
                let record = entity_index.get_record_mut(self.get_entity_unchecked(row)).unwrap();
                record.row = row;

                // TODO: check if this is necessary.
                self.entities.add(last).write(0);
            }
            else {
                // Simply drop the values in the last row
                for (i, col) in self.columns.iter().enumerate() {
                    let ti = &col.type_info;
                    let size = ti.size();
                    let row_ptr = col.data.add(row * size);

                    match ti.hooks.drop_fn {
                        Some(drop) if should_drop[i] => drop(row_ptr),
                        _ => {},
                    }
                }

                // TODO: check if this is necessary.
                self.entities.add(row).write(0);
            }
        }

        self.len -= 1;
    }
}

impl Drop for ArchetypeData {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }

        unsafe {
            let entt_layout = Layout::array::<Entity>(self.cap).expect("Invalid layout");
            let entt_ptr = self.entities.as_ptr() as *mut u8;
            std::alloc::dealloc(entt_ptr, entt_layout);

            let flags_layout = Layout::array::<EntityFlags>(self.cap).expect("Invalid layout");
            let flags_ptr = self.flags.as_ptr() as *mut u8;
            std::alloc::dealloc(flags_ptr, flags_layout);

            for col in self.columns.iter() {
                let size = col.type_info.size();

                if let Some(drop) = col.type_info.hooks.drop_fn {
                    let mut ptr = col.data;

                    for _ in 0..self.len {
                        drop(ptr);
                        ptr = ptr.add(size)
                    }
                }
                
                let align = col.type_info.align();
                let layout = Layout::from_size_align(self.cap * size, align).expect("Invalid layout");
                std::alloc::dealloc(col.data.as_ptr(), layout);
            };
        }
    }
}