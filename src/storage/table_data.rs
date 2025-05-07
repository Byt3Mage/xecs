use crate::{
    component::ComponentValue, entity::Entity, entity_index::EntityIndex,
    types::type_info::TypeInfo,
};
use const_assert::const_assert;
use std::{
    alloc::Layout,
    ptr::{self, NonNull},
    rc::Rc,
};

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
    /// Component id that owns this column.
    id: Entity,
    pub(super) data: NonNull<u8>,
    pub(super) type_info: Rc<TypeInfo>,
}

impl Column {
    pub fn new(id: Entity, type_info: Rc<TypeInfo>) -> Self {
        Self {
            id,
            data: NonNull::dangling(),
            type_info,
        }
    }

    #[inline]
    pub(crate) fn id(&self) -> Entity {
        self.id
    }

    /// #Safety
    /// Caller must ensure that `row` is valid for this column.
    #[inline]
    pub(super) unsafe fn add_ptr(&self, row: usize) -> NonNull<u8> {
        // SAFETY:
        // data is non-null
        // caller guarantees row is valid.
        unsafe { self.data.add(row * self.type_info.size()) }
    }

    unsafe fn drop(&mut self, len: usize, cap: usize) {
        unsafe {
            let (size, align) = self.type_info.size_align();
            let layout = Layout::from_size_align(size * cap, align).unwrap();

            if let Some(drop_fn) = self.type_info.drop_fn {
                let mut ptr = self.data;
                for _ in 0..len {
                    drop_fn(ptr);
                    ptr = ptr.add(size)
                }
            }

            std::alloc::dealloc(self.data.as_ptr(), layout);
        }
    }
}

impl TypeErased for Column {
    unsafe fn alloc(&mut self, new_cap: usize) {
        let (size, align) = self.type_info.size_align();
        let layout = Layout::from_size_align(new_cap * size, align).expect("Invalid layout");
        let ptr = unsafe { std::alloc::alloc(layout) };

        self.data = match NonNull::new(ptr) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(layout),
        };
    }

    unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        debug_assert!(new_cap > old_cap, "tried to realloc with smaller capacity");

        let (size, align) = self.type_info.size_align();
        let old_layout = Layout::from_size_align(old_cap * size, align).expect("Invalid layout");
        let new_layout = Layout::from_size_align(new_cap * size, align).expect("Invalid layout");
        let old_ptr = self.data.as_ptr();
        let new_ptr = unsafe { std::alloc::realloc(old_ptr, old_layout, new_layout.size()) };

        self.data = match NonNull::new(new_ptr) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(new_layout),
        };
    }
}

impl TypeErased for NonNull<Entity> {
    unsafe fn alloc(&mut self, new_cap: usize) {
        let layout = Layout::array::<Entity>(new_cap).expect("Invalid laout");
        let ptr = unsafe { std::alloc::alloc(layout) };

        *self = match NonNull::new(ptr as *mut Entity) {
            Some(p) => p,
            None => std::alloc::handle_alloc_error(layout),
        };
    }

    unsafe fn realloc(&mut self, old_cap: usize, new_cap: usize) {
        debug_assert!(new_cap > old_cap, "realloc with smaller capacity");

        let old_layout = Layout::array::<Entity>(old_cap).expect("Invalid layout");
        let new_layout = Layout::array::<Entity>(new_cap).expect("Invalid layout");
        let old_ptr = self.as_ptr().cast();
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
    // - The caller must ensure that `a` and `b` are valid rows.
    // - a and b are guaranteed not to overlap, since they are different rows.
    unsafe {
        let base = entities.as_ptr();
        let ap = base.add(a);
        let bp = base.add(b);
        std::ptr::swap_nonoverlapping(ap, bp, 1);
    }
}

unsafe fn drop_entities(entities: &mut NonNull<Entity>, cap: usize) {
    const_assert!(
        || !std::mem::needs_drop::<Entity>(),
        "Entity type must not require drop, otherwise implement drop"
    );
    let layout = Layout::array::<Entity>(cap).expect("Invalid layout");
    unsafe { std::alloc::dealloc(entities.as_ptr().cast(), layout) };
}

pub(crate) struct TableData {
    pub(super) entities: NonNull<Entity>,
    pub(super) columns: Box<[Column]>,
    len: usize,
    cap: usize,
}

impl Drop for TableData {
    fn drop(&mut self) {
        if self.cap == 0 {
            return;
        }

        unsafe {
            drop_entities(&mut self.entities, self.cap);

            for col in self.columns.iter_mut() {
                col.drop(self.len, self.cap);
            }
        }
    }
}

impl TableData {
    pub(crate) fn new(columns: Box<[Column]>) -> Self {
        Self {
            entities: NonNull::dangling(),
            columns,
            len: 0,
            cap: 0,
        }
    }

    /// Returns number of rows in this table.
    #[inline]
    pub(crate) fn len(&self) -> usize {
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
                self.columns
                    .iter_mut()
                    .for_each(|col| col.alloc(required_cap));
            } else {
                self.entities.realloc(self.cap, required_cap);
                self.columns
                    .iter_mut()
                    .for_each(|col| col.realloc(self.cap, required_cap));
            };
        }

        self.cap = required_cap;
    }

    fn grow(&mut self) {
        let new_cap = if self.cap == 0 {
            4
        } else {
            self.cap.checked_mul(2).expect("Capacity overflow")
        };

        self.reserve(new_cap - self.cap);
    }

    /// Creates a new row without initializing its elements.
    /// This function will grow all columns if necessary.
    ///
    /// # Safety
    /// - The rows for the new entity in all columns will be uninitialized (hence, unsafe).
    /// - The caller must ensure to initialize the new row in all columns before using it.
    pub(crate) unsafe fn new_row_uninit(&mut self, entity: Entity) -> usize {
        // TODO: check if I should use `[Self::grow]` instead
        if self.len == self.cap {
            self.grow();
        }

        // SAFETY:
        // * Pointer offset properly calculated.
        // * NonNull ptr safe to write.
        unsafe {
            self.entities.as_ptr().add(self.len).write(entity);
        }

        let row = self.len;
        self.len += 1;
        row
    }

    /// Sets the value of the component at `row`, in `column`.
    ///
    /// This function does not perform bounds or type checking.
    ///
    /// # Safety
    /// - The caller ensures that `row` and `column` are valid.
    /// - The caller ensures that the type matches.
    /// - The caller ensures that the row is initialized.
    pub(crate) unsafe fn set_unchecked<C: ComponentValue>(
        &self,
        column: usize,
        row: usize,
        val: C,
    ) {
        debug_assert!(row < self.len, "row out of bounds");
        debug_assert!(column < self.columns.len(), "column out of bounds");

        // SAFETY:
        // - The caller ensures that `row` and `column` are valid.
        // - The caller ensures that the type `C` matches the column.
        // - The caller ensures that the row is initialized.
        unsafe {
            let col = self.columns.get_unchecked(column);
            debug_assert!(col.type_info.is::<C>(), "TableData: type mismatch");
            let ptr = col.add_ptr(row).as_ptr().cast();
            let _ = ptr::replace(ptr, val);
        }
    }

    /// Sets the value of the component at `row`, in `column`.
    ///
    /// This function does not perform bounds or type checking.
    ///
    /// # Safety
    /// - The caller ensures that `row` and `column` are valid.
    /// - The caller ensures that the type matches.
    /// - The caller ensures that the row is uinitialized.
    pub(crate) unsafe fn set_unitialized<C: ComponentValue>(
        &self,
        column: usize,
        row: usize,
        val: C,
    ) {
        // SAFETY:
        // - The caller ensures that `row` and `column` are valid.
        // - The caller ensures that the type `C` matches the column.
        // - The caller ensures that the row is uninitialized.
        unsafe {
            let col = self.columns.get_unchecked(column);
            debug_assert!(col.type_info.is::<C>(), "TableData: type mismatch");
            let ptr = col.add_ptr(row).as_ptr().cast();
            ptr::write(ptr, val);
        }
    }

    /// Returns a reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds or type checking.
    ///
    /// # Safety
    /// - The caller ensures that `row` and `column` are valid.
    /// - The caller ensures that the type matches.
    pub(crate) unsafe fn get_unchecked<C: ComponentValue>(&self, column: usize, row: usize) -> &C {
        debug_assert!(row < self.len, "row out of bounds");
        debug_assert!(column < self.columns.len(), "column out of bounds");
        // SAFETY:
        // - The caller ensures that `row` and `column` are valid.
        // - The caller ensures that the type `C` matches the column.
        unsafe {
            let col = self.columns.get_unchecked(column);
            debug_assert!(col.type_info.is::<C>(), "TableData: type mismatch");
            col.add_ptr(row).cast().as_ref()
        }
    }

    /// Returns a mutable reference to the element at `row`, in `column`.
    ///
    /// This function does not perform bounds checking.
    ///
    /// # Safety
    /// - The caller ensures that `row` and `column` are valid.
    /// - The caller ensures that the type matches.
    pub(crate) unsafe fn get_unchecked_mut<C: ComponentValue>(
        &mut self,
        column: usize,
        row: usize,
    ) -> &mut C {
        debug_assert!(row < self.len, "row out of bounds");
        debug_assert!(column < self.columns.len(), "column out of bounds");
        // SAFETY:
        // - The caller ensures that `row` and `column` in bounds.
        // - The caller ensures that the type `C` matches the column.
        unsafe {
            let col = self.columns.get_unchecked_mut(column);
            assert!(col.type_info.is::<C>(), "TableData: type mismatch");
            col.add_ptr(row).cast().as_mut()
        }
    }

    /// Returns the entity at the specified `row`.
    ///
    /// # Safety
    /// - The row must be in-bounds (`row` < `self.len`).
    pub(crate) unsafe fn get_entity_unchecked(&self, row: usize) -> &Entity {
        debug_assert!(row < self.len, "row out of bounds");
        // SAFETY: The caller ensures that `row` is valid.
        unsafe { self.entities.add(row).as_ref() }
    }

    /// Deletes the row by swapping with the last row
    /// and returns the entity that was in the last row
    /// or `None` if `row` was the last.
    ///
    /// # Safety
    /// - `row` must be in bounds.
    /// - `drop_check` and `self.columns` must have the same length
    pub(super) unsafe fn delete_row(&mut self, row: usize, drop_check: &[bool]) -> Option<Entity> {
        debug_assert!(row < self.len, "row out of bounds");
        debug_assert!(self.columns.len() == drop_check.len());

        let last = self.len - 1;

        let last_entity = unsafe {
            if row != last {
                swap_entities(&mut self.entities, row, last);

                for (col, &should_drop) in self.columns.iter().zip(drop_check) {
                    let ti = &col.type_info;
                    let size = ti.size();
                    let row_ptr = col.data.add(row * size);
                    let last_ptr = col.data.add(last * size);

                    if should_drop {
                        if let Some(drop_fn) = ti.drop_fn {
                            (drop_fn)(row_ptr)
                        }
                    }

                    ptr::copy_nonoverlapping(last_ptr.as_ptr(), row_ptr.as_ptr(), size);
                }

                Some(*self.get_entity_unchecked(row))
            } else {
                // Simply drop the values in the last row
                for (col, &should_drop) in self.columns.iter().zip(drop_check) {
                    let ti = &col.type_info;

                    if should_drop {
                        if let Some(drop_fn) = ti.drop_fn {
                            (drop_fn)(col.data.add(row * ti.size()))
                        }
                    }
                }

                None
            }
        };

        self.len -= 1;
        last_entity
    }
}
