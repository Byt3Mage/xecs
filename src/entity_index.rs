use crate::{entity::Entity, error::EntityIndexError, flags::EntityFlags, table_index::TableId};
use std::{alloc::Layout, usize};

const PAGE_BITS: usize = 12;
const PAGE_SIZE: usize = 1 << PAGE_BITS;
const PAGE_MASK: usize = PAGE_SIZE - 1;

#[inline(always)]
const fn page_index(e: Entity) -> usize {
    (e.id() as usize) >> PAGE_BITS
}

#[inline(always)]
const fn page_offset(e: Entity) -> usize {
    (e.id() as usize) & PAGE_MASK
}

pub(crate) struct EntityRecord {
    pub(crate) table: TableId,
    pub(crate) row: usize,
    pub(crate) flags: EntityFlags,
    dense: usize,
}

struct Page {
    records: Box<[EntityRecord]>,
    is_active: bool,
}

impl Page {
    fn new() -> Page {
        Self {
            records: Box::from([]),
            is_active: false,
        }
    }

    fn set_active(&mut self) -> &mut Self {
        if self.is_active {
            return self;
        }

        self.records = unsafe {
            let layout = Layout::array::<EntityRecord>(PAGE_SIZE).expect("invalid layout");
            let ptr = std::alloc::alloc_zeroed(layout) as *mut EntityRecord;

            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }

            Box::from_raw(std::slice::from_raw_parts_mut(ptr, PAGE_SIZE))
        };
        self.is_active = true;
        self
    }
}

pub struct EntityIndex {
    entities: Vec<Entity>,
    pages: Vec<Page>,
    alive_count: usize,
    max_id: u64,
}

impl EntityIndex {
    pub(crate) fn new() -> Self {
        Self {
            entities: vec![Entity::NULL],
            pages: vec![],
            alive_count: 1,
            max_id: 0,
        }
    }

    /// Ensures the page is allocated for the index.
    /// Does not take in [self] due to borrowing issues.
    fn ensure_page(pages: &mut Vec<Page>, page_index: usize) -> &mut Page {
        if page_index >= pages.len() {
            pages.resize_with(page_index + 1, Page::new);
        }
        // Allocate a new page if not already created
        pages[page_index].set_active()
    }

    /// Returns the [EntityRecord] for the [Entity].
    ///
    /// [Entity] must exist and must be alive to have a record.
    pub(crate) fn get_record(&self, entity: Entity) -> Result<&EntityRecord, EntityIndexError> {
        match self.pages.get(page_index(entity)) {
            Some(page) if page.is_active => {
                let record = &page.records[page_offset(entity)];

                if record.dense == 0 {
                    return Err(EntityIndexError::NonExistent(entity));
                }

                if record.dense >= self.alive_count || self.entities[record.dense] != entity {
                    return Err(EntityIndexError::NotAlive(entity));
                }

                Ok(record)
            }
            _ => Err(EntityIndexError::NonExistent(entity)),
        }
    }

    /// Returns the mutable [EntityRecord] for the [Entity].
    ///
    /// [Entity] must exist and must be alive to have a record.
    pub(crate) fn get_record_mut(
        &mut self,
        entity: Entity,
    ) -> Result<&mut EntityRecord, EntityIndexError> {
        match self.pages.get_mut(page_index(entity)) {
            Some(page) if page.is_active => {
                let record = &mut page.records[page_offset(entity)];

                if record.dense == 0 {
                    return Err(EntityIndexError::NonExistent(entity));
                }

                if record.dense >= self.alive_count || self.entities[record.dense] != entity {
                    return Err(EntityIndexError::NotAlive(entity));
                }

                Ok(record)
            }
            _ => Err(EntityIndexError::NonExistent(entity)),
        }
    }

    /// Checks if the [Entity] is alive
    pub fn is_alive(&self, entity: Entity) -> bool {
        match self.pages.get(page_index(entity)) {
            Some(page) if page.is_active => {
                let dense = page.records[page_offset(entity)].dense;
                dense != 0 && dense < self.alive_count && self.entities[dense] == entity
            }
            _ => false,
        }
    }

    /// Check if entity id was ever created (whether alive or dead).
    pub fn exists(&self, entity: Entity) -> bool {
        match self.pages.get(page_index(entity)) {
            Some(page) if page.is_active => page.records[page_offset(entity)].dense != 0,
            _ => false,
        }
    }

    pub(crate) fn remove_id(&mut self, entity: Entity) {
        let page = match self.pages.get_mut(page_index(entity)) {
            Some(page) if page.is_active => page,
            _ => return,
        };

        let record = &mut page.records[page_offset(entity)];
        let dense = record.dense;

        // Do nothing entity is already dead or nonexistent.
        if dense == 0 || dense >= self.alive_count || self.entities[dense] != entity {
            return;
        }

        self.alive_count -= 1;

        record.table = TableId::NULL;
        record.row = usize::MAX;
        record.dense = self.alive_count;
        record.flags = EntityFlags::empty();

        let last_index = self.alive_count;
        let last_entity =
            std::mem::replace(&mut self.entities[last_index], entity.inc_generation());

        // swap last alive entity with removed entity.
        if dense != last_index {
            let last_page = &mut self.pages[page_index(last_entity)];
            assert!(last_page.is_active);
            let last_record = &mut last_page.records[page_offset(last_entity)];

            debug_assert!(
                last_record.dense == last_index,
                "INTERNAL ERROR: entity index corrupted"
            );

            last_record.dense = dense;
            self.entities[dense] = last_entity;
        }

        debug_assert!(
            !self.is_alive(entity),
            "INTERNAL ERROR: entity index corrupted"
        );
    }

    pub(crate) fn new_id(&mut self) -> Entity {
        if self.alive_count < self.entities.len() {
            // Recycle id.
            let entity = self.entities[self.alive_count];
            self.alive_count += 1;
            return entity;
        }

        // Create new id.
        self.max_id += 1;
        let new_entity = Entity::from_raw(self.max_id);

        // Ensure we haven't exceeded allowed number of entities
        assert!(
            self.max_id <= (u32::MAX as u64),
            "max id {new_entity} exceeds 32 bits",
        );

        // Ensure id hasn't been issued before.
        debug_assert!(
            !self.exists(new_entity),
            "new entity id:({}) already in use (likely due to overlapping ranges)",
            new_entity.id()
        );

        self.entities.push(new_entity);

        let page = Self::ensure_page(&mut self.pages, page_index(new_entity));
        let record = &mut page.records[page_offset(new_entity)];

        record.table = TableId::NULL;
        record.row = usize::MAX;
        record.dense = self.alive_count;
        record.flags = EntityFlags::empty();

        self.alive_count += 1;

        debug_assert!(self.alive_count == self.entities.len());

        new_entity
    }

    #[inline]
    pub fn alive_count(&self) -> usize {
        self.alive_count - 1
    }

    #[inline]
    pub fn dead_count(&self) -> usize {
        self.entities.len() - self.alive_count
    }
}
