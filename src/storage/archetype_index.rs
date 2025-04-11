use std::{mem::{ManuallyDrop, MaybeUninit}, rc::Rc};
use crate::{component::ComponentLocation, graph::GraphNode, storage::archetype::Archetype, type_info::Type, world::World};

use super::{archetype_data::Column, archetype_flags::ArchetypeFlags};

/// Stable, non-recycled handle into [ArchetypeIndex].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub(crate) struct ArchetypeId {
    idx: u32,
    ver: u32,
}

impl ArchetypeId {
    pub const fn null() -> Self {
        Self {
            idx: core::u32::MAX,
            ver: 1
        }
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

    pub fn new() -> Self {
        Self::with_capacity(0)
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
        self.slots
        .get(id.idx as usize)
        .filter(|slot| slot.ver == id.ver)
        // SAFETY: slot is occupied.
        .map(|slot| unsafe { &(*slot.data.arch) })
    }
 
    /// Returns a mutable reference to the corresponding [Archetype].
    #[inline]
    pub fn get_mut(&mut self, id: ArchetypeId) -> Option<&mut Archetype> {
        self.slots
        .get_mut(id.idx as usize)
        .filter(|slot| slot.ver == id.ver)
        // SAFETY: slot is occupied.
        .map(|slot| unsafe { &mut (*slot.data.arch) })
    }

    /// Returns disjoint mutable references to the corresponding [Archetype]s.
    /// 
    /// # Panics
    /// This function panics if any of the ids are invalid or overlapping.
    /// This method is similar to `[core::slice::get_disjoint_mut]`.
    pub fn get_multi_mut<const N: usize>(&mut self, ids: [ArchetypeId; N]) -> [&mut Archetype; N] {
        // See: https://doc.rust-lang.org/std/primitive.slice.html#method.get_disjoint_unchecked_mut
        let mut archetypes: MaybeUninit<[&mut Archetype; N]> = MaybeUninit::uninit();
        let ptr = archetypes.as_mut_ptr();
        let len = self.slots.len();

        unsafe {
            for i in 0..N {
                // SAFETY: index `i` is in bounds.
                let id = ids.get_unchecked(i);
                let idx = id.idx as usize;
    
                // Check if id is valid.
                if idx >= len { panic!("Archetype id: {id:?} is invalid"); }
    
                // SAFETY: index `idx` is in bounds.
                let slot =  self.slots.get_unchecked_mut(idx);
    
                // Check if id is occupied.
                if slot.ver != id.ver { panic!("Archetype id: {id:?} is invalid"); }
    
                // Check if ids are overlapping.
                for id2 in &ids[..i] { if idx == id2.idx as usize { panic!("Archetype ids overlapping"); } }
                
                // SAFETY: id is valid and non-overlapping.
                ptr.cast::<&mut Archetype>().add(i).write(&mut (*slot.data.arch));
            }
            
            // SAFETY: all ids are valid and non-overlapping.
            archetypes.assume_init()
        }
    }
}

pub(crate) struct ArchetypeBuilder<'a> {
    world: &'a mut World,
    flags: ArchetypeFlags,
    ty: Type,
    node: GraphNode,
}

impl <'a> ArchetypeBuilder<'a> {
    pub(crate) fn new(world: &'a mut World, type_ids: Type) -> Self {
        Self {
            world,
            flags: ArchetypeFlags::empty(),
            ty: type_ids,
            node: GraphNode::new(),
        }
    }

    pub(crate) fn flags(mut self, flags: ArchetypeFlags) -> Self {
        self.flags = flags;
        self
    }

    pub(crate) fn with_flag(mut self, flag: ArchetypeFlags) -> Self {
        self.flags |= flag;
        self
    }

    pub(crate) fn build(self) -> ArchetypeId {
        self.world.archetypes.insert_with_id(|arch_id| {
            let id_count = self.ty.id_count();
            let mut columns = Vec::with_capacity(id_count);
            let mut component_map = Vec::with_capacity(id_count);

            for (idx, id) in self.ty.iter().enumerate() {
                let mut location = ComponentLocation{ id_index: idx, id_count: 1, column_index: None };
    
                // Component contains type_info, initialize a column for it.
                if let Some(type_info)  = self.world.type_infos.get(id) {
                    columns.push(Column::new(Rc::clone(type_info), None));
    
                    let col_idx = Some(columns.len() - 1);
                    component_map.push(col_idx);
                    location.column_index = col_idx
                }
    
                let component_record = self.world.component_index.get_mut(id).unwrap();
    
                component_record.archetypes.insert(arch_id, location);
            }

            todo!()
        })
    }
}