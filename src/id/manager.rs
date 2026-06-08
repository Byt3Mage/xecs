use crate::{id::Id, table_index::TableId};

#[derive(Clone, Copy)]
pub struct IdRecord {
    pub(crate) table: TableId,
    pub(crate) row: usize,
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

    pub(crate) fn set_location(&mut self, id: Id, table: TableId, row: usize) {
        if let Some(&dense) = self.sparse.get(id.idx() as usize)
            && dense < self.alive_count
        {
            let entry = &mut self.dense[dense];
            if entry.id == id {
                entry.record.table = table;
                entry.record.row = row
            }
        }
    }

    /// Returns the [IdRecord] for the [Id].
    ///
    /// [Id] must exist and must be alive to have a record.
    pub(crate) fn get(&self, id: Id) -> Option<IdRecord> {
        let dense = *self.sparse.get(id.idx() as usize)?;

        if dense >= self.alive_count {
            return None;
        }

        let entry = &self.dense[dense];
        (entry.id == id).then_some(entry.record)
    }

    /// Checks if the [Id] is alive
    pub fn is_alive(&self, id: Id) -> bool {
        self.sparse
            .get(id.idx() as usize)
            .is_some_and(|&d| d < self.alive_count && self.dense[d].id == id)
    }

    /// Check if [Id] was ever created (whether alive or dead).
    pub fn exists(&self, id: Id) -> bool {
        self.sparse
            .get(id.idx() as usize)
            .is_some_and(|&d| d < self.dense.len())
    }

    pub(crate) fn remove_id(&mut self, id: Id) {
        let sparse = id.idx() as usize;

        let Some(&dense) = self.sparse.get(sparse) else {
            return;
        };

        // Do nothing entity if already dead or nonexistent.
        if dense >= self.alive_count || self.dense[dense].id != id {
            return;
        }

        self.alive_count -= 1;
        self.sparse[sparse] = self.alive_count;
        self.dense[dense].id = id.next_version();

        // swap last alive entity with removed entity.
        if dense != self.alive_count {
            self.dense.swap(dense, self.alive_count);
            self.sparse[self.dense[dense].id.idx() as usize] = dense;
        }

        debug_assert!(!self.is_alive(id), "XECS INTERNAL ERROR: IdManager corrupted");
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
        assert!(self.max_id <= (u32::MAX as u64), "max id {new_id} exceeds 32 bits",);

        // Ensure id hasn't been issued before.
        debug_assert!(!self.exists(new_id), "new id: `{new_id}` already in use");

        self.dense.push(Entry { id: new_id, record: f(new_id) });

        let sparse = new_id.idx() as usize;

        if sparse >= self.sparse.len() {
            self.sparse.resize(sparse + 1, usize::MAX);
        }

        self.sparse[sparse] = self.alive_count;
        self.alive_count += 1;

        debug_assert!(self.alive_count == self.dense.len());

        new_id
    }

    #[inline]
    pub fn num_alive(&self) -> usize {
        self.alive_count
    }

    #[inline]
    pub fn num_dead(&self) -> usize {
        self.dense.len() - self.alive_count
    }
}
