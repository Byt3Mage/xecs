use std::rc::Rc;

use crate::{
    component::Component,
    data_structures::ErasedVec,
    flags::TableFlags,
    graph::GraphNode,
    id::{Entity, Id, IdMap, Signature, entity_manager::EntityLocation},
    table_index::TableId,
    type_info::TypeInfo,
    type_traits::Data,
    world::World,
};

pub(crate) struct Column {
    id: Entity,
    data: ErasedVec,
}

impl Column {
    pub(crate) fn new(id: Entity, type_info: Rc<TypeInfo>) -> Self {
        Self {
            id,
            data: ErasedVec::new(type_info),
        }
    }
}

pub(crate) struct TableData {
    ids: Vec<Entity>,
    columns: Box<[Column]>,
}

impl TableData {
    pub(crate) fn new(columns: Box<[Column]>) -> Self {
        Self {
            ids: vec![],
            columns,
        }
    }

    #[inline(always)]
    pub(crate) fn ids(&self) -> &[Entity] {
        &self.ids
    }

    /// Returns number of rows in this table.
    #[inline]
    pub(crate) fn row_count(&self) -> usize {
        self.ids.len()
    }

    /// Creates a new row without initializing its elements.
    /// This function will grow all columns uniformly, if necessary.
    ///
    /// # Safety
    /// - The rows for the new id in all columns will be uninitialized (hence, unsafe).
    /// - The caller must ensure to write to all the columns in the new row.
    pub(crate) unsafe fn new_row(&mut self, id: Entity) -> usize {
        let row = self.ids.len();
        self.ids.push(id);
        row
    }

    /// # Safety
    /// - `row` must be in bounds
    /// - `drop_check` must have the same length as `self.columns`
    pub(super) unsafe fn delete_row(&mut self, row: usize, drop_check: &[bool]) -> Option<Entity> {
        debug_assert!(row < self.ids.len(), "TableData: row out of bounds");
        debug_assert!(drop_check.len() == self.columns.len());

        for (col, &should_drop) in self.columns.iter_mut().zip(drop_check) {
            todo!("Delete row");
        }

        let removed = self.ids.swap_remove(row);

        if row == self.ids.len() {
            None
        } else {
            Some(removed)
        }
    }
}

pub(crate) struct Table {
    /// Handle to self in [TableIndex](super::table_index::TableIndex).
    pub(crate) id: TableId,
    /// Flags describing the capabilites of this table
    pub(crate) _flags: TableFlags,
    /// Vector of component [Entity] ids
    pub(crate) signature: Signature,
    /// Storage for component data.
    pub(crate) data: TableData,
    /// Maps keys to columns indices.
    pub(crate) column_map: IdMap<usize>,
    /// Node representation for traversals.
    pub(crate) node: GraphNode,
}

impl Table {
    pub(crate) fn validate_data(&self) {
        #[cfg(debug_assertions)]
        {
            let len = self.data.row_count();

            self.data
                .columns
                .iter()
                .for_each(|col| assert_eq!(len, col.data.len()));
        }
    }

    /// Gets a reference to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    /// - `T` must be the value type of the column.
    #[inline(always)]
    pub(crate) unsafe fn get<T>(&self, col_id: &impl Id, row: usize) -> Option<&T>
    where
        T: Component<DataType = Data>,
    {
        // SAFETY:
        // - col is valid for this table.
        // - callers ensure row is valid for the table.
        // - callers ensure T is the value type of the column.
        col_id
            .map_get(&self.column_map)
            .map(|&col| unsafe { self.data.columns.get_unchecked(col).data.get(row) })
    }

    /// Gets a reference to the component of an entity.
    ///
    /// # Safety
    /// - `row` must be valid in this table.
    /// - `T` must be the value type of the column.
    #[inline(always)]
    pub(crate) unsafe fn get_mut<T>(&mut self, row: usize, col_id: &impl Id) -> Option<&mut T>
    where
        T: Component<DataType = Data>,
    {
        // SAFETY:
        // - col is valid for this table.
        // - callers ensure row is valid for the table.
        // - callers ensure T is the value type of the column.
        col_id
            .map_get(&self.column_map)
            .map(|&col| unsafe { self.data.columns.get_unchecked_mut(col).data.get_mut(row) })
    }
}

/// Moves `id` from src table to dst.
/// Returns the row in dst table.
///
/// # Safety
/// - `src_row` must be a valid row in `src`.
/// - `src` and `dst` must not be the same table.
pub(crate) unsafe fn move_id(
    world: &mut World,
    id: Entity,
    src: TableId,
    src_row: usize,
    dst: TableId,
) {
    let (src, dst) = world.tables.get_2_mut(src, dst).unwrap();

    debug_assert!(src_row < src.data.row_count(), "row out of bounds");

    // Append a new row to the destination table, but don't initialize columns.
    let dst_row = unsafe { dst.data.new_row(id) };
    let src_columns = &mut src.data.columns;
    let dst_columns = &mut dst.data.columns;
    let mut drop_check = vec![true; src_columns.len()];

    for (i_src, src_col) in src_columns.iter_mut().enumerate() {
        if let Some(&i_dst) = src_col.id.map_get(&dst.column_map) {
            todo!()
        } else {
            // Component not in destination table.
            // TODO: Emit remove hooks
        }
    }

    // update the record of the id swapped into src_row.
    if let Some(i) = unsafe { src.data.delete_row(src_row, &drop_check) } {
        world.entity_manager.set_location(
            i,
            EntityLocation {
                table: src.id, // set table just to be pendatic, not really necessary.
                row: src_row,
            },
        );
    }

    // update record of moved entity.
    world.entity_manager.set_location(
        id,
        EntityLocation {
            table: dst.id,
            row: dst_row,
        },
    );
}
