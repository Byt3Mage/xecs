use std::collections::HashMap;

use crate::{component::ComponentRecord, id::Id};

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
        self.components.contains_key(&id)
    }

    #[inline(always)]
    pub(crate) fn get(&self, id: Id) -> Option<&ComponentRecord> {
        self.components.get(&id)
    }

    #[inline(always)]
    pub(crate) fn get_mut(&mut self, id: Id) -> Option<&mut ComponentRecord> {
        self.components.get_mut(&id)
    }

    #[inline(always)]
    pub(crate) fn insert(&mut self, id: Id, cr: ComponentRecord) -> Option<ComponentRecord> {
        self.components.insert(id, cr)
    }
}