use crate::{
    data_structures::SparseIndex, error::InvalidId, flags::IdFlags, id::Id, table_index::TableId,
};

#[derive(Clone, Copy)]
pub struct IdLocation {
    pub(crate) table: TableId,
    pub(crate) row: usize,
}

pub(crate) struct IdRecord {
    pub(crate) location: IdLocation,
    pub(crate) flags: IdFlags,
}

struct Entry {
    id: Id,
    record: IdRecord,
}

pub struct IdManager {
    dense: Vec<Entry>,
    sparse: Vec<usize>,
    alive_count: usize,
    max_id: u64,
}

impl IdManager {
    pub(crate) fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
            alive_count: 0,
            max_id: 0,
        }
    }

    /// Returns the `table` and `row` for the [Id].
    ///
    /// [Id] must exist and must be alive to have a record.
    pub(crate) fn get_location(&self, id: Id) -> Result<IdLocation, InvalidId> {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) if dense < self.alive_count => {
                // SAFETY: we just checked that dense is in bounds,
                // and we guarantee that alive_count <= dense.len().
                let entry = unsafe { self.dense.get_unchecked(dense) };

                if entry.id != id {
                    return Err(InvalidId(id));
                }

                Ok(entry.record.location)
            }
            _ => Err(InvalidId(id)),
        }
    }

    pub(crate) fn set_location(&mut self, id: Id, location: IdLocation) {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) if dense < self.alive_count => {
                // SAFETY: we just checked that dense is in bounds,
                // and we guarantee that alive_count is always <= entities.len().
                let entry = unsafe { self.dense.get_unchecked_mut(dense) };

                if entry.id == id {
                    entry.record.location = location;
                }
            }
            _ => {}
        }
    }

    /// Returns the [IdRecord] for the [Id].
    ///
    /// [Id] must exist and must be alive to have a record.
    pub(crate) fn get_record(&self, id: Id) -> Result<&IdRecord, InvalidId> {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) if dense < self.alive_count => {
                // SAFETY: we just checked that dense is in bounds,
                // and we guarantee that alive_count <= dense.len().
                let entry = unsafe { self.dense.get_unchecked(dense) };

                if entry.id != id {
                    return Err(InvalidId(id));
                }

                Ok(&entry.record)
            }
            _ => Err(InvalidId(id)),
        }
    }

    /// Returns the mutable [IdRecord] for the [Id].
    ///
    /// [Id] must be alive to have a record.
    pub(crate) fn get_record_mut(&mut self, id: Id) -> Result<&mut IdRecord, InvalidId> {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) if dense < self.alive_count => {
                // SAFETY: we just checked that dense is in bounds,
                // and we guarantee that alive_count is always <= entities.len().
                let entry = unsafe { self.dense.get_unchecked_mut(dense) };

                if entry.id != id {
                    return Err(InvalidId(id));
                }

                Ok(&mut entry.record)
            }
            _ => Err(InvalidId(id)),
        }
    }

    /// Checks if the [Entity] is alive
    pub fn is_alive(&self, id: Id) -> bool {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) if dense < self.alive_count => {
                // SAFETY: We just checked that dense is in bounds.
                unsafe { self.dense.get_unchecked(dense) }.id == id
            }
            _ => false,
        }
    }

    /// Check if id was ever created (whether alive or dead).
    pub fn exists(&self, id: Id) -> bool {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) => dense < self.dense.len(),
            _ => false,
        }
    }

    /// Get the current version of the entity id
    /// Returns `None` if the id is not valid.
    pub fn get_current(&self, id: Id) -> Option<Id> {
        match self.sparse.get(id.to_sparse_index()) {
            Some(&dense) if dense < self.alive_count => {
                // SAFETY: We just checked that dense is in bounds.
                Some(unsafe { self.dense.get_unchecked(dense).id })
            }
            _ => None,
        }
    }

    pub(crate) fn remove_id(&mut self, id: Id) {
        let sparse = id.to_sparse_index();
        let Some(&dense) = self.sparse.get(sparse) else {
            return;
        };

        // Do nothing entity is already dead or nonexistent.
        if dense >= self.alive_count || self.dense[dense].id != id {
            return;
        }

        self.alive_count -= 1;
        self.sparse[sparse] = self.alive_count;
        self.dense[dense].id = id.inc_gen();

        // swap last alive entity with removed entity.
        if dense != self.alive_count {
            self.dense.swap(dense, self.alive_count);
            self.sparse[self.dense[dense].id.to_sparse_index()] = dense;
        }

        debug_assert!(!self.is_alive(id), "INTERNAL ERROR: IdIndex corrupted");
    }

    pub(crate) fn new_id(&mut self, f: impl FnOnce(Id) -> IdRecord) -> Id {
        if self.alive_count < self.dense.len() {
            // Recycle id.
            let entry = &mut self.dense[self.alive_count];
            entry.record = f(entry.id);
            self.alive_count += 1;

            return entry.id;
        }

        // Create new id.
        let new_id = Id::from_raw(self.max_id);
        self.max_id += 1;

        // Ensure we haven't exceeded allowed number of entities
        assert!(
            self.max_id <= (u32::MAX as u64),
            "max id {new_id} exceeds 32 bits",
        );

        // Ensure id hasn't been issued before.
        debug_assert!(
            !self.exists(new_id),
            "new id:({}) already in use (likely due to overlapping ranges)",
            new_id.index()
        );

        self.dense.push(Entry {
            id: new_id,
            record: f(new_id),
        });

        let sparse = new_id.to_sparse_index();

        if sparse >= self.sparse.len() {
            self.sparse.resize(sparse + 1, usize::MAX);
        }

        self.sparse[sparse] = self.alive_count;
        self.alive_count += 1;

        debug_assert!(self.alive_count == self.dense.len());

        new_id
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.alive_count - 1
    }

    #[inline]
    pub fn dead_count(&self) -> usize {
        self.dense.len() - self.alive_count
    }
}
