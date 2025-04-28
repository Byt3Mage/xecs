use crate::{
    entity::Entity,
    entity_index::EntityIndex,
    pointer::{Ptr, PtrMut},
    storage::swap_entities,
};
use const_assert::const_assert;
use std::{alloc::Layout, ptr::NonNull};

use super::{Column, TypeErased};

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

        const_assert!(
            || std::mem::needs_drop::<Entity>() == false,
            "Entity type must not require drop, fix drop otherwise"
        );

        unsafe {
            let entt_layout = Layout::array::<Entity>(self.cap).expect("Invalid layout");
            let entt_ptr = self.entities.as_ptr() as *mut u8;
            std::alloc::dealloc(entt_ptr, entt_layout);

            for col in self.columns.iter() {
                let (size, align) = col.type_info.size_align();
                let layout =
                    Layout::from_size_align(self.cap * size, align).expect("Invalid layout");

                if let Some(drop_fn) = col.type_info.hooks.drop_fn {
                    let mut ptr = col.data;

                    for _ in 0..self.len {
                        drop_fn(ptr);
                        ptr = ptr.add(size)
                    }
                }

                std::alloc::dealloc(col.data.as_ptr(), layout);
            }
        }
    }
}

impl TableData {
    pub fn new(columns: Box<[Column]>) -> Self {
        Self {
            entities: NonNull::dangling(),
            columns,
            len: 0,
            cap: 0,
        }
    }

    /// Returns number of rows in this table.
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
        self.reserve(new_cap);
    }

    /// Creates a new row without initializing its elements.
    /// This function will grow all columns if necessary.
    ///
    /// # Safety
    /// - The rows for the new entity in all columns will be uninitialized (hence, unsafe).
    /// - The caller must ensure to initialize the new row in all columns before using it.
    pub unsafe fn new_row_uninit(&mut self, entity: Entity) -> usize {
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

    /// Returns a [Ptr] to the element at `row`, in `column`, without doing bounds checking.
    ///
    /// # Safety
    /// - The caller ensures that `column` is valid.
    /// - The caller ensures that `row` is valid.
    pub unsafe fn get_unchecked(&self, column: usize, row: usize) -> Ptr {
        debug_assert!(column < self.columns.len(), "column out of bounds");
        debug_assert!(row < self.len, "row out of bounds");
        // SAFETY: The caller ensures that `row` and `column` are valid.
        unsafe { self.columns.get_unchecked(column).get(row) }
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
        unsafe { self.columns.get_unchecked_mut(column).get_mut(row) }
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

    /// Deletes the row by swapping with the last row
    /// and returns the entity that was in the last row
    /// or `None` if `row` was the last.
    pub(super) fn delete_row(
        &mut self,
        entity_index: &mut EntityIndex,
        row: usize,
        should_drop: Vec<bool>,
    ) {
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
                        _ => {}
                    }

                    (ti.hooks.move_fn)(last_ptr, row_ptr)
                }

                // Update entity record.
                // Allowed to panic since last row must contain a valid entity.
                let record = entity_index
                    .get_record_mut(self.get_entity_unchecked(row))
                    .unwrap();
                record.row = row;

                // TODO: check if this is necessary.
                self.entities.add(last).write(Entity::NULL);
            } else {
                // Simply drop the values in the last row
                for (i, col) in self.columns.iter().enumerate() {
                    let ti = &col.type_info;
                    let size = ti.size();
                    let row_ptr = col.data.add(row * size);

                    match ti.hooks.drop_fn {
                        Some(drop) if should_drop[i] => drop(row_ptr),
                        _ => {}
                    }
                }

                // TODO: check if this is necessary.
                self.entities.add(row).write(Entity::NULL);
            }
        }

        self.len -= 1;
    }
}
