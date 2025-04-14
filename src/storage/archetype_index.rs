use std::{mem::ManuallyDrop, fmt::Display, rc::Rc};
use crate::{component::ComponentLocation, graph::GraphNode, storage::archetype::Archetype, type_info::Type, world::World};

use super::{archetype_data::{ArchetypeData, Column}, archetype_flags::ArchetypeFlags};

/// Stable, non-recycled handle into [ArchetypeIndex].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub(crate) struct ArchetypeId {
    idx: u32,
    ver: u32,
}

impl Display for ArchetypeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ArchetypeId({}, v{})", self.idx, self.ver)
    }
}

impl ArchetypeId {
    pub const fn null() -> Self {
        Self {
            idx: core::u32::MAX,
            ver: 1
        }
    }

    #[inline]
    pub const fn is_null(&self) -> bool {
        self.idx == core::u32::MAX
    }
}

/// Either contains [Archetype] data or the next free index.
union SlotData {
    arch: ManuallyDrop<Archetype>,
    next_free: u32,
}

struct Slot {
    data: SlotData,
    // Even value means vacant, odd value means occupied.
    ver: u32,
}

pub(crate) struct ArchetypeIndex {
    slots: Vec<Slot>,
    free_head: u32,
    len: u32,
}

impl ArchetypeIndex {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: Vec::with_capacity(capacity),
            free_head: 0,
            len: 0
        }
    }

    fn insert_with_id<F>(&mut self, f: F) -> ArchetypeId 
    where 
        F: FnOnce(ArchetypeId) -> Archetype
    {
        // Don't modify index until we have the value in case of panics.
        let new_len = self.len + 1;

        if new_len == core::u32::MAX {
            panic!("Max number of archetypes reached")
        }

        if let Some(slot) = self.slots.get_mut(self.free_head as usize) {
            let generation = slot.ver | 1;

            // Recycle free id.
            let id = ArchetypeId{ idx: self.free_head, ver: generation };
            let value = f(id);

            // SAFETY: occupied_generation is odd, 
            // so archetype can be assigned to slot data.
            unsafe {
                self.free_head = slot.data.next_free;
                slot.data.arch = ManuallyDrop::new(value);
                slot.ver = generation;
            }

            self.len = new_len;
            
            return id;
        }

        // Create new id.
        let generation = 1;
        let id = ArchetypeId{idx: self.slots.len() as u32, ver: generation};
        let value = f(id);
        
        // Create new slot before adjusting free head in case f or the allocation panics or errors.
        self.slots.push(Slot { data: SlotData{ arch: ManuallyDrop::new(value) }, ver: generation });
        self.free_head = id.idx + 1;
        self.len = new_len;

        id
    }

    /// Removes and returns the [Archetype] corresponding to the handle if it exists.
    pub(crate) fn remove(&mut self, id: ArchetypeId) -> Option<Archetype> {
        self.slots
        .get_mut(id.idx as usize)
        .filter(|slot| slot.ver == id.ver)
        .map(|slot| {
            // SAFETY: slot is occupied, so data contains an archetype.
            let archetype = unsafe { ManuallyDrop::take(&mut slot.data.arch) };

            slot.data.next_free = self.free_head;
            slot.ver += 1;
            
            self.free_head = id.idx;
            self.len -= 1;

            archetype
        })
    }

    /// Returns a reference to the corresponding [Archetype].
    pub fn get(&self, id: ArchetypeId) -> Option<&Archetype> {
        let idx = id.idx as usize;

        if idx >= self.slots.len() {
            return None;
        }

        // SAFETY: 
        // - we just did a bounds check on idx.
        // - we check that versions match, so slot must contain data.
        unsafe {
            let slot = self.slots.get_unchecked(idx);
            if slot.ver == id.ver { Some(& (*slot.data.arch)) } else { None }
        }
    }
 
    /// Returns a mutable reference to the corresponding [Archetype].
    #[inline]
    pub fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        let idx = id.idx as usize;

        if idx >= self.slots.len() {
            return None;
        }

        // SAFETY: 
        // - we just did a bounds check on idx.
        // - we check that versions match, so slot must contain data.
        unsafe {
            let slot = self.slots.get_unchecked_mut(idx);
            if slot.ver == id.ver { Some(&mut (*slot.data.arch)) } else { None }
        }
    }

    /// Returns two disjoint mutable references to the corresponding [Archetype]s.
    /// 
    /// # Panics
    /// This function panics if `a` or `b` are invalid or if they overlap.
    pub fn get_two_mut(&mut self, a: ArchetypeId, b: ArchetypeId) -> (&mut Archetype, &mut Archetype) {  
        let (a_idx, b_idx) = (a.idx as usize, b.idx as usize);

        if a_idx == b_idx { panic!("Ids overlap"); }

        let len = self.slots.len();

        if a_idx >= len || b_idx >= len { panic!("Invalid id(s)"); }

        // SAFETY:
        // - we checked ids don't overlap.
        // - we checked ids are in bounds.
        unsafe {
            let a_slot = &mut (*self.slots.as_mut_ptr().add(a_idx));
            let b_slot = &mut (*self.slots.as_mut_ptr().add(b_idx));
        
            if a_slot.ver != a.ver || b_slot.ver != b.ver { panic!("Invalid id(s)"); }

            (&mut(*a_slot.data.arch), &mut(*b_slot.data.arch))
        }
    }
}

pub(crate) struct ArchetypeBuilder<'a> {
    world: &'a mut World,
    flags: ArchetypeFlags,
    type_: Type,
    node: GraphNode,
}

impl <'a> ArchetypeBuilder<'a> {
    pub(crate) fn new(world: &'a mut World, type_ids: Type) -> Self {
        Self {
            world,
            flags: ArchetypeFlags::empty(),
            type_: type_ids,
            node: GraphNode::new(),
        }
    }

    pub(crate) fn with_flags(mut self, flags: ArchetypeFlags) -> Self {
        self.flags |= flags;
        self
    }

    pub(crate) fn build(self) -> ArchetypeId {
        self.world.archetypes.insert_with_id(|arch_id| {
            let ty_count = self.type_.id_count();
            let mut columns = Vec::with_capacity(ty_count);
            let mut component_map = Vec::with_capacity(ty_count);
            let mut column_map = Vec::with_capacity(ty_count);

            for (idx, id) in self.type_.iter().enumerate() {
                let mut location = ComponentLocation{ id_index: idx, id_count: 1, column_index: None };
    
                // Component contains type_info, initialize a column for it.
                if let Some(type_info)  = self.world.type_infos.get(id) {
                    columns.push(Column::new(Rc::clone(type_info)));
                    column_map.push(idx);
                    
                    let col_idx = Some(columns.len() - 1);
                    location.column_index = col_idx;
                    component_map.push(col_idx);
                }
    
                // TODO: create component record.
                let component_record = self.world.components.get_mut(id).unwrap();
    
                component_record.archetypes.insert(arch_id, location);
            }

            Archetype {
                id: arch_id,
                flags: self.flags,
                type_: self.type_,
                component_map: component_map.into(),
                column_map: column_map.into(),
                node: self.node,
                data: ArchetypeData::new(columns.into()),
            }
        })
    }
}