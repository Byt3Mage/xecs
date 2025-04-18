use std::{alloc::Layout, usize};

use crate::{entity::Entity, entity_flags::EntityFlags, error::EntityIndexError, id::{generation, GENERATION_MASK}, storage::archetype_index::ArchetypeId};

const PAGE_BITS: usize = 12;
const PAGE_SIZE: usize = 1 << PAGE_BITS;
const PAGE_MASK: usize = PAGE_SIZE - 1;

#[inline(always)]
const fn to_id(e: Entity) -> (usize, usize) {
    let id = (e as u32) as usize;
    (id >> PAGE_BITS, id & PAGE_MASK)
}

#[inline]
pub const fn increment_generation(e: Entity) -> Entity {
    (e & !GENERATION_MASK) | ((0xFFFF & (generation(e) + 1)) << 32)
}

pub(crate) struct EntityRecord {
    pub cr: Option<u64>,
    pub arch: ArchetypeId,
    pub row: usize,
    dense: usize,
    pub flags: EntityFlags,
}

struct Page {
    records: [EntityRecord; PAGE_SIZE] 
}

impl Page {
    fn alloc() -> Box<Page> {
        // zeroed allocation used to avoid initializing a large array on the stack
        let layout  = Layout::new::<Page>();

        unsafe {
            let ptr= std::alloc::alloc_zeroed(layout);

            if ptr.is_null() { 
                std::alloc::handle_alloc_error(layout); 
            }

            // SAFETY: ptr is checked for null.
            Box::from_raw(ptr as *mut Page)
        }
    }
}

type PagePtr = Option<Box<Page>>;

pub(crate) struct EntityIndex {
    entities: Vec<Entity>,
    pages: Vec<PagePtr>,
    alive_count: usize,
    max_id: u64,
}

impl EntityIndex {
    pub(crate) fn new() -> Self {
        Self{
            entities: vec![0 as Entity],
            pages: vec![],
            alive_count: 1,
            max_id: 0,
        }
    }

    /// Ensures the page is allocated for the index.
    /// Does not take in [self] due to borrowing issues.
    fn ensure_page(pages: &mut Vec<PagePtr>, page_index: usize) -> &mut Page {
        if page_index >= pages.len() {
            pages.resize_with(page_index + 1, ||None);
        }

        // Allocate a new page if the pointer is null
        let page = pages[page_index].get_or_insert_with(Page::alloc);

        page.as_mut()
    }

    #[inline]
    fn get_page(&self, page_index: usize) -> Option<&Page> {
        self.pages.get(page_index).and_then(Option::as_deref)
    }

    /// Returns the [EntityLocation] for the [Entity].
    /// 
    /// [Entity] must exist and must be alive to have a location.
    pub(crate) fn get_location(&self, entity: Entity) -> Result<(ArchetypeId, usize), EntityIndexError> {
        let (page_index, record_index) = to_id(entity); 
        let page = self.get_page(page_index).ok_or(EntityIndexError::NonExistent(entity))?;
        let record = &page.records[record_index];
    
        if record.dense == 0 {
            return Err(EntityIndexError::NonExistent(entity));
        }
        
        if record.dense >= self.alive_count || self.entities[record.dense] != entity {
            return Err(EntityIndexError::NotAlive(entity));
        }

        Ok((record.arch, record.row))
    }

    /// Returns the [EntityRecord] for the [Entity].
    /// 
    /// [Entity] must exist but may not be alive.
    pub(crate) fn get_any_location(&self, entity: Entity) -> Option<(ArchetypeId, usize)> {
        let (page_index, record_index) = to_id(entity);
        let page = self.get_page(page_index)?; 
        let record = &page.records[record_index];
        (record.dense != 0).then_some((record.arch, record.row))
    }

    /// Returns the [EntityRecord] for the [Entity].
    /// 
    /// [Entity] must exist but may not be alive.
    pub(crate) fn get_any_record(&self, entity: Entity) -> Option<&EntityRecord> {
        let (page_index, record_index) = to_id(entity);
        let page = self.get_page(page_index)?;       
        let record = &page.records[record_index];
        (record.dense != 0).then_some(record)
    }

    /// Returns the mutable [EntityRecord] for the [Entity].
    /// 
    /// [Entity] must exist but may not be alive.
    pub(crate) fn get_any_record_mut(&mut self, entity: Entity) -> Option<&mut EntityRecord> {
        let (page_index, record_index) = to_id(entity);
        let page = self.pages.get_mut(page_index)?.as_mut()?;       
        let record = &mut page.records[record_index];
        (record.dense != 0).then_some(record)
    }

    /// Returns the [EntityRecord] for the [Entity].
    /// 
    /// [Entity] must exist and must be alive to have a record.
    pub(crate) fn get_record(&self, entity: Entity) -> Result<&EntityRecord, EntityIndexError> {
        let (page_index, record_index) = to_id(entity); 
        let page = self.get_page(page_index).ok_or(EntityIndexError::NonExistent(entity))?;
        let record = &page.records[record_index];
    
        if record.dense == 0 {
            return Err(EntityIndexError::NonExistent(entity));
        }
        
        if record.dense >= self.alive_count || self.entities[record.dense] != entity {
            return Err(EntityIndexError::NotAlive(entity));
        }

        Ok(record)
    }

    /// Returns the mutable [EntityRecord] for the [Entity].
    /// 
    /// [Entity] must exist and must be alive to have a record.
    pub(crate) fn get_record_mut(&mut self, entity: Entity) -> Result<&mut EntityRecord, EntityIndexError> {
        let (page_index, record_index) = to_id(entity); 

        let page = match self.pages.get_mut(page_index).and_then(Option::as_deref_mut) {
            Some(p) => p,
            None => return Err(EntityIndexError::NonExistent(entity)),
        };

        let record = &mut page.records[record_index];
    
        if record.dense == 0 {
            return Err(EntityIndexError::NonExistent(entity));
        }
        
        if record.dense >= self.alive_count || self.entities[record.dense] != entity {
            return Err(EntityIndexError::NotAlive(entity));
        }

        Ok(record)
    }

    /// Set the entity's location. Does nothing if the entity is dead or nonexistent.
    pub(crate) fn set_location(&mut self, entity: Entity, arch: ArchetypeId, row: usize) {
        if let Ok(record) = self.get_record_mut(entity) {
            record.arch = arch;
            record.row = row
        }
    }

    /// Gets the entity with the current generation encoded.
    pub fn get_current(&self, entity: u64) -> Entity {
        self.get_record(entity).map_or(0, |r|self.entities[r.dense])
    }

    /// Checks if the [Entity] is alive
    pub fn is_alive(&self, entity: Entity) -> bool {
        let (page_index, record_index) = to_id(entity);
        let Some(page) = self.get_page(page_index) else { return false };
        let Some(record) = page.records.get(record_index) else { return false };
    
        record.dense != 0 && 
        record.dense < self.alive_count &&
        self.entities[record.dense] == entity
    }

    /// Check if entity id was ever created (whether alive or dead).
    pub fn exists(&self, id: u32) -> bool {
        let id = id as usize;
        let Some(page) = self.get_page(id & PAGE_BITS) else { return false; };
        page.records[id & PAGE_MASK].dense != 0
    }

    pub(crate) fn remove_id(&mut self, entity: Entity) {
        let (page_index, record_index) = to_id(entity);
        let Some(page) = self.pages.get_mut(page_index).and_then(Option::as_deref_mut) else { return };
        let record = &mut page.records[record_index];
        let dense = record.dense;

        // Do nothing entity is already dead or nonexistent.
        if dense == 0 || record.dense >= self.alive_count || self.entities[dense] != entity {
            return;
        }
        
        let last_index = { self.alive_count -= 1; self.alive_count };

        record.cr = None;
        record.arch = ArchetypeId::null();
        record.row = 0;
        record.dense = last_index;

        let last_entity = std::mem::replace(&mut self.entities[last_index], increment_generation(entity)); 
        
        // swap last alive entity with removed entity.
        if dense != last_index {
            let last_record = self.get_any_record_mut(last_entity).expect("INTERNAL ERROR: entity index corrupted");
            
            debug_assert!(last_record.dense == last_index, "INTERNAL ERROR: entity index corrupted");

            last_record.dense = dense;
            self.entities[dense] = last_entity;
        }
        
        debug_assert!(!self.is_alive(entity), "INTERNAL ERROR: entity index corrupted");
    }

    pub(crate) fn new_id(&mut self) -> Entity {
        if self.alive_count < self.entities.len() {
            // Recycle id.
            let new_index = self.alive_count; self.alive_count += 1;
            
            return self.entities[new_index];
        }
    
        // Create new id.
        let new_entity = { self.max_id += 1; self.max_id };
        
        // Ensure we haven't exceeded allowed number of entities
        assert!(self.max_id as u32 <= u32::MAX, "max id {} exceeds 32 bits", self.max_id);

        let id: u32 = new_entity as u32;

        // Ensure id hasn't been issued before.
        debug_assert!(!self.exists(id), "new entity {} id already in use (likely due to overlapping ranges)", id);

        self.entities.push(new_entity);
       
        let page = Self::ensure_page(&mut self.pages, (id as usize) >> PAGE_BITS);
        let record = &mut page.records[(id as usize) & PAGE_MASK];
        
        record.cr = None;
        record.arch = ArchetypeId::null();
        record.row = 0;
        record.dense = self.alive_count; self.alive_count += 1;
        
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