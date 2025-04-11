use std::{collections::HashMap, marker::PhantomData};

use crate::{component_flags::ComponentFlags, id::Id, storage::archetype_index::ArchetypeId, world::WorldRef};

pub trait ComponentValue: 'static {}

pub struct ComponentView<'a, T: ComponentValue> {
    id: Id,
    world: WorldRef<'a>,
    _phantom: PhantomData<fn()-> T>
}

impl <'a, T: ComponentValue> ComponentView<'a, T> {
    pub(crate) fn new(world: impl Into<WorldRef<'a>>, id: Id) -> Self {
        Self {
            id,
            world: world.into(),
            _phantom: Default::default()
        }
    }
}

/// Component location info within an [Archetype](crate::storage::archetype::Archetype).
pub(crate) struct ComponentLocation {
    /// First index of id within the archetype's [Type](crate::type_info::Type).
    pub id_index: usize,
    /// Number of times the id occurs in the archetype. E.g id, (id, \*), (\*, id).
    pub id_count: usize,
    /// First [Column](crate::storage::archetype_data::Column) index where the id appears (if not tag).
    pub column_index: Option<usize>,
}

pub struct ComponentRecord {
    pub id: Id,
    pub flags: ComponentFlags,
    pub archetypes: HashMap<ArchetypeId, ComponentLocation>
}

impl ComponentRecord {
    pub fn new(id: Id) -> Self {
        Self {
            id,
            flags: ComponentFlags::empty(),
            archetypes: HashMap::new(),
        }
    }
}