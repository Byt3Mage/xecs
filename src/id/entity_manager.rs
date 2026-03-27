use crate::{data_structures::SparseIndex, flags::EntityFlags, id::Entity, table_index::TableId};

#[derive(Clone, Copy)]
pub struct EntityLocation {
    pub(crate) table: TableId,
    pub(crate) row: usize,
}

pub(crate) struct EntityRecord {
    pub(crate) location: EntityLocation,
    pub(crate) flags: EntityFlags,
}

struct Entry {
    entity: Entity,
    record: EntityRecord,
}

pub struct EntityManager {
    dense: Vec<Entry>,
    sparse: Vec<usize>,
    alive_count: usize,
    max_id: u64,
}

impl EntityManager {
    pub(crate) fn new() -> Self {
        Self {
            dense: vec![],
            sparse: vec![],
            alive_count: 0,
            max_id: 0,
        }
    }

    /// Returns the `table` and `row` for the [Entity].
    ///
    /// [Entity] must exist and must be alive to have a record.
    pub(crate) fn get_location(&self, e: Entity) -> Option<EntityLocation> {
        let dense = *self.sparse.get(e.idx())?;

        if dense >= self.alive_count {
            return None;
        }

        let entry = &self.dense[dense];
        (entry.entity == e).then_some(entry.record.location)
    }

    pub(crate) fn set_location(&mut self, e: Entity, location: EntityLocation) {
        if let Some(&dense) = self.sparse.get(e.idx())
            && dense < self.alive_count
        {
            let entry = &mut self.dense[dense];

            if entry.entity == e {
                entry.record.location = location;
            }
        }
    }

    /// Returns the [IdRecord] for the [Id].
    ///
    /// [Id] must exist and must be alive to have a record.
    pub(crate) fn get_record(&self, e: Entity) -> Option<&EntityRecord> {
        let dense = *self.sparse.get(e.idx())?;

        if dense >= self.alive_count {
            return None;
        }

        let entry = &self.dense[dense];
        (entry.entity == e).then_some(&entry.record)
    }

    /// Returns the mutable [IdRecord] for the [Id].
    ///
    /// [Id] must be alive to have a record.
    pub(crate) fn get_record_mut(&mut self, e: Entity) -> Option<&mut EntityRecord> {
        let dense = *self.sparse.get(e.idx())?;

        if dense >= self.alive_count {
            return None;
        }

        let entry = &mut self.dense[dense];
        (entry.entity == e).then_some(&mut entry.record)
    }

    /// Checks if the [Entity] is alive
    pub fn is_alive(&self, e: Entity) -> bool {
        self.sparse
            .get(e.idx())
            .is_some_and(|&d| d < self.alive_count && self.dense[d].entity == e)
    }

    /// Check if entity was ever created (whether alive or dead).
    pub fn exists(&self, e: Entity) -> bool {
        self.sparse
            .get(e.idx())
            .is_some_and(|&d| d < self.dense.len())
    }

    pub(crate) fn remove_id(&mut self, e: Entity) {
        let sparse = e.idx();
        let Some(&dense) = self.sparse.get(sparse) else {
            return;
        };

        // Do nothing entity is already dead or nonexistent.
        if dense >= self.alive_count || self.dense[dense].entity != e {
            return;
        }

        self.alive_count -= 1;
        self.sparse[sparse] = self.alive_count;
        self.dense[dense].entity = e.inc_ver();

        // swap last alive entity with removed entity.
        if dense != self.alive_count {
            self.dense.swap(dense, self.alive_count);
            self.sparse[self.dense[dense].entity.idx()] = dense;
        }

        debug_assert!(!self.is_alive(e), "INTERNAL ERROR: EntityManager corrupted");
    }

    pub(crate) fn new_id(&mut self, f: impl FnOnce(Entity) -> EntityRecord) -> Entity {
        if self.alive_count < self.dense.len() {
            // Recycle id.
            let entry = &mut self.dense[self.alive_count];
            entry.record = f(entry.entity);
            self.alive_count += 1;

            return entry.entity;
        }

        // Create new id.
        let new_id = Entity::from_raw(self.max_id);
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
            new_id.idx()
        );

        self.dense.push(Entry {
            entity: new_id,
            record: f(new_id),
        });

        let sparse = new_id.idx();

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
        self.alive_count - 1
    }

    #[inline]
    pub fn num_dead(&self) -> usize {
        self.dense.len() - self.alive_count
    }
}
