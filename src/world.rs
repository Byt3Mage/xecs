use std::{collections::HashMap, ops::{Deref, DerefMut}, rc::Rc};
use const_assert::const_assert;
use crate::{
    component::{ComponentRecord, ComponentValue, TypedComponentBuilder, TypedComponentView},
    entity::Entity,
    entity_index::EntityIndex,
    entity_view::EntityView,
    error::EcsResult,
    graph::archetype_traverse_add,
    id::Id,
    storage::{
        archetype::{move_entity, move_entity_to_root},
        archetype_index::{ArchetypeId, ArchetypeIndex},
    },
    type_info::{Type, TypeInfo, TypeMap},
};

pub struct World {
    pub(crate) entity_index: EntityIndex,
    pub(crate) component_index: HashMap<Id, ComponentRecord>,
    pub(crate) type_infos: HashMap<Id, Rc<TypeInfo>>,
    pub(crate) archetypes: ArchetypeIndex,
    pub(crate) archetype_map: HashMap<Type, ArchetypeId>,
    pub(crate) root_arch: ArchetypeId,
    pub(crate) type_ids: TypeMap,
}

impl World {
    pub fn new() -> Self {
        Self {
            entity_index: EntityIndex::new(),
            component_index: HashMap::new(),
            type_infos: HashMap::new(),
            archetypes: ArchetypeIndex::with_capacity(100),
            archetype_map: HashMap::new(),
            root_arch: ArchetypeId::null(),
            type_ids: TypeMap::new(),
        }
    }

    pub fn component<C: ComponentValue>(&mut self) -> Result<TypedComponentView<C>, TypedComponentBuilder<C>> {
        match self.type_ids.get_id_t::<C>() {
            Some(id) => Ok(TypedComponentView::new(self, id)),
            None => Err(TypedComponentBuilder::new()),
        }
    }

    pub fn new_entity(&mut self) -> EntityView {
        let entity = self.entity_index.new_id();
        move_entity_to_root(self, entity);
        EntityView::new(self, entity)
    }

    pub fn add_id(&mut self, entity: Entity, id: Id) -> EcsResult<()> {
        let location = self.entity_index.get_location(entity)?;
        let src_arch = location.arch;
        let dst_arch = archetype_traverse_add(self,src_arch, id);

        if src_arch != dst_arch {
            // SAFETY:
            // - src_row is valid in enitity index.
            // - we just checked that src_arch and dst_arch are not the same.
            unsafe {
                move_entity(self, entity, src_arch, location.row, dst_arch)
            }
        }

        Ok(())
    }

    /// Checks if entity has the component.
    pub fn has_t<C: ComponentValue>(&self, entity: Entity) -> bool {
        let id = self.type_ids.get_id_t::<C>().unwrap();
        self.has(entity, id)
    }
    
    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Id) -> bool {
        // TODO: check if I want to bubble up an error or assert on manual registration.
        let Ok(location) = self.entity_index.get_location(entity) else { return false };
        let component = &self.component_index[&id];
        component.archetypes.contains_key(&location.arch)
    }
}

pub struct WorldRef<'world> {
    world: &'world mut World
}

impl Deref for WorldRef<'_> {
    type Target = World;

    fn deref(&self) -> &Self::Target {
        self.world
    }
}

impl DerefMut for WorldRef<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.world
    }
}

impl <'world> From<&'world mut World> for WorldRef<'world> {
    fn from(value: &'world mut World) -> Self {
        Self {
            world: value
        }
    }
}