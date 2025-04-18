use std::collections::HashMap;

use crate::{component::ComponentRecord, entity::{ECS_ANY, ECS_WILDCARD}, id::{is_pair, pair, pair_first, pair_second, strip_generation, Id, COMPONENT_MASK}};

pub(crate) struct ComponentIndex {
    components: HashMap<Id, ComponentRecord>
}

impl ComponentIndex {
    pub(crate) fn new() -> Self {
        Self {
            components: HashMap::new()
        }
    }

    #[inline(always)]
    pub(crate) fn contains(&self, id: Id) -> bool {
        self.components.contains_key(&component_hash(id))
    }

    #[inline(always)]
    pub(crate) fn get(&self, id: Id) -> Option<&ComponentRecord> {
        self.components.get(&component_hash(id))
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self, id: Id) -> Option<&mut ComponentRecord> {
        self.components.get_mut(&component_hash(id))
    }

    #[inline(always)]
    pub(crate) fn insert(&mut self, id: Id, cr: ComponentRecord) -> Option<ComponentRecord> {
        self.components.insert(component_hash(id), cr)
    }
}

pub(crate) const fn component_hash(id: Id) -> Id {
    let id = strip_generation(id);

    if is_pair(id) {
        let mut rel = pair_first(id & COMPONENT_MASK) as u64;
        let mut obj = pair_second(id) as u64;

        if rel == ECS_ANY {
            rel = ECS_WILDCARD;
        }

        if obj == ECS_ANY {
            obj = ECS_WILDCARD;
        }

        return pair(rel, obj);
    }

    id
}