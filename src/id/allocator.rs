use crate::{id::Id, table_storage::TableId};

/// Error returned if accessing an [IdRecord] fails
#[derive(thiserror::Error, Debug)]
#[error("{0} is not alive")]
pub struct NotAlive(pub Id);

#[derive(thiserror::Error, Debug)]
#[error("allocator has exhasuted all available ids")]
pub struct IdsExhausted;

#[derive(Debug, Clone, Copy)]
pub(crate) struct IdRecord {
    pub(crate) table: TableId,
    pub(crate) row: u32,
}

struct Entry {
    id: Id,
    record: IdRecord,
}

/// Sparse set of ids. `dense[..alive]` holds live ids, `dense[alive..]`
/// holds dead ids awaiting reuse. `sparse` maps an id's index to its
/// position in `dense`; both always have one slot per id ever created.
pub(crate) struct IdAllocator {
    sparse: Vec<u32>,
    dense: Vec<Entry>,
    alive: u32,
}

impl IdAllocator {
    pub(crate) fn new() -> Self {
        Self { sparse: vec![], dense: vec![], alive: 0 }
    }

    #[inline]
    pub(crate) fn set_location(&mut self, id: Id, r: IdRecord) {
        debug_assert!(self.is_alive(id), "set_location on dead id {id}");
        self.dense[self.sparse[id.index() as usize] as usize].record = r;
    }

    /// Returns the [IdRecord] for the [Id].
    ///
    /// [Id] must be alive to have a record.
    pub(crate) fn get(&self, id: Id) -> Result<IdRecord, NotAlive> {
        self.sparse
            .get(id.index() as usize)
            .filter(|&&d| d < self.alive)
            .map(|&d| &self.dense[d as usize])
            .filter(|e| e.id == id)
            .map(|e| e.record)
            .ok_or(NotAlive(id))
    }

    /// Checks if the [Id] is alive
    #[inline]
    pub fn is_alive(&self, id: Id) -> bool {
        self.sparse
            .get(id.index() as usize)
            .is_some_and(|&d| d < self.alive && (self.dense[d as usize].id == id))
    }

    /// Validates that the [Id] is alive
    #[inline]
    pub fn validate_alive(&self, id: Id) -> Result<(), NotAlive> {
        if self.is_alive(id) { Ok(()) } else { Err(NotAlive(id)) }
    }

    pub(crate) fn remove_id(&mut self, id: Id) {
        let sparse = id.index() as usize;

        let Some(&dense) = self.sparse.get(sparse) else {
            return;
        };

        let dense_idx = dense as usize;

        // Do nothing entity if already dead or nonexistent.
        if dense >= self.alive || self.dense[dense_idx].id != id {
            return;
        }

        self.alive -= 1;
        self.sparse[sparse] = self.alive;
        self.dense[dense_idx].id = id.next_generation();

        // swap last alive entity with removed entity.
        if dense != self.alive {
            self.dense.swap(dense_idx, self.alive as usize);
            self.sparse[self.dense[dense_idx].id.index() as usize] = dense;
        }

        debug_assert!(!self.is_alive(id), "XECS: IdAllocator corrupted");
    }

    /// Fails unless `n` more ids can be allocated.
    pub(crate) fn ensure_available(&self, n: u32) -> Result<(), IdsExhausted> {
        let fresh = (Id::MAX_INDEX + 1).saturating_sub(self.dense.len() as u32);
        let available = self.num_dead() + fresh;
        if n <= available { Ok(()) } else { Err(IdsExhausted) }
    }

    pub(crate) fn new_id(&mut self, f: impl FnOnce(Id) -> IdRecord) -> Result<Id, IdsExhausted> {
        // Recycle id if a free one is available.
        if let Some(entry) = self.dense.get_mut(self.alive as usize) {
            entry.record = f(entry.id);
            self.alive += 1;
            return Ok(entry.id);
        }

        let idx = match u32::try_from(self.dense.len()) {
            Ok(i) if i <= Id::MAX_INDEX => i,
            _ => return Err(IdsExhausted),
        };

        let id = Id::new(idx);
        self.dense.push(Entry { id, record: f(id) });
        self.sparse.push(self.alive);
        self.alive += 1;
        Ok(id)
    }

    #[inline]
    pub fn num_live(&self) -> u32 {
        self.alive
    }

    #[inline]
    pub fn num_dead(&self) -> u32 {
        (self.dense.len() as u32) - self.alive
    }
}
