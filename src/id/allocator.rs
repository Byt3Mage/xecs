use crate::{id::Id, table_index::TableId};

/// Error returned if accessing an [IdRecord](crate::id::manager::IdRecord) fails
#[derive(thiserror::Error, Debug)]
#[error("{0} is not alive")]
pub struct NotAlive(pub Id);

#[derive(Debug, Clone, Copy)]
pub struct IdRecord {
    pub(crate) table: TableId,
    pub(crate) row: u32,
}

struct Entry {
    id: Id,
    record: IdRecord,
}

pub struct IdAllocator {
    sparse: Vec<usize>,
    dense: Vec<Entry>,
    alive: usize,
    max: u32,
}

impl IdAllocator {
    pub(crate) fn new() -> Self {
        Self { sparse: vec![], dense: vec![], alive: 0, max: 0 }
    }

    pub(crate) fn set_location(&mut self, id: Id, table: TableId, row: u32) {
        if let Some(&dense) = self.sparse.get(id.index() as usize)
            && dense < self.alive
        {
            let entry = &mut self.dense[dense];
            if entry.id == id {
                entry.record.table = table;
                entry.record.row = row
            }
        } else {
            debug_assert!(false, "set_location on stale id")
        }
    }

    /// Returns the [IdRecord] for the [Id].
    ///
    /// [Id] must be alive to have a record.
    pub(crate) fn get(&self, id: Id) -> Result<IdRecord, NotAlive> {
        self.sparse
            .get(id.index() as usize)
            .and_then(|dense| (dense < &self.alive).then(|| &self.dense[*dense]))
            .and_then(|entry| (entry.id == id).then_some(entry.record))
            .ok_or(NotAlive(id))
    }

    /// Checks if the [Id] is alive
    pub fn is_alive(&self, id: Id) -> bool {
        self.sparse
            .get(id.index() as usize)
            .is_some_and(|&d| d < self.alive && self.dense[d].id == id)
    }

    /// Check if [Id] was ever created (whether alive or dead).
    pub fn exists(&self, id: Id) -> bool {
        self.sparse
            .get(id.index() as usize)
            .is_some_and(|&d| d < self.dense.len())
    }

    pub(crate) fn remove_id(&mut self, id: Id) {
        let sparse = id.index() as usize;

        let Some(&dense) = self.sparse.get(sparse) else {
            return;
        };

        // Do nothing entity if already dead or nonexistent.
        if dense >= self.alive || self.dense[dense].id != id {
            return;
        }

        self.alive -= 1;
        self.sparse[sparse] = self.alive;
        self.dense[dense].id = id.next_generation();

        // swap last alive entity with removed entity.
        if dense != self.alive {
            self.dense.swap(dense, self.alive);
            self.sparse[self.dense[dense].id.index() as usize] = dense;
        }

        debug_assert!(!self.is_alive(id), "XECS INTERNAL ERROR: IdAllocator corrupted");
    }

    pub(crate) fn new_id(&mut self, f: impl FnOnce(Id) -> IdRecord) -> Id {
        if let Some(entry) = self.dense.get_mut(self.alive) {
            // Recycle id if a free one is available.
            entry.record = f(entry.id);
            self.alive += 1;
            return entry.id;
        }

        // Create new id.
        assert!(
            self.max < Id::MAX_INDEX,
            "id index {} exceeds limit(={})",
            self.max,
            Id::MAX_INDEX
        );

        let new_id = Id::new(self.max);
        self.max += 1;

        debug_assert!(!self.exists(new_id), "new id: `{new_id}` already in use");

        self.dense.push(Entry { id: new_id, record: f(new_id) });

        let sparse = new_id.index() as usize;

        if sparse >= self.sparse.len() {
            self.sparse.resize(sparse + 1, usize::MAX);
        }

        self.sparse[sparse] = self.alive;
        self.alive += 1;

        debug_assert!(self.alive == self.dense.len());

        new_id
    }

    #[inline]
    pub fn num_alive(&self) -> usize {
        self.alive
    }

    #[inline]
    pub fn num_dead(&self) -> usize {
        self.dense.len() - self.alive
    }
}
