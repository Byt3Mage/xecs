use std::{collections::HashMap, ops::{Deref, DerefMut}, rc::Rc};
use const_assert::const_assert;

use crate::{
    component::{ComponentRecord, ComponentValue, ComponentView}, entity::Entity, entity_index::{EntityIndex, EntityLocation}, entity_view::EntityView, error::{EcsError, EcsResult, EntityCreateError}, graph::archetype_traverse_add, id::Id, storage::archetype_index::{ArchetypeId, ArchetypeIndex}, type_info::{Type, TypeInfo, TypeMap}
};

pub struct World {
    pub(crate) entity_index: EntityIndex,
    pub(crate) component_index: HashMap<Id, ComponentRecord>,
    pub(crate) type_infos: HashMap<Id, Rc<TypeInfo>>,
    pub(crate) archetypes: ArchetypeIndex,
    pub(crate) archetype_map: HashMap<Type, ArchetypeId>,
    pub(crate) root_archetype: ArchetypeId,
    pub(crate) named_ids: HashMap<Rc<str>, Id>,
    pub(crate) type_ids: TypeMap,
}

impl World {
    /// Registers a new component or returns existing one
    pub fn component<T: ComponentValue>(&mut self) -> ComponentView<T> {
        let mut is_new = false;
        let &mut id = self.type_ids.entry::<T>().or_insert_with(|| { is_new = true; self.entity_index.new_id() });
        
        if is_new {
            self.component_index.insert(id, ComponentRecord::new(id));
        }
        
        //TODO: complete registration.
        ComponentView::new(self, id)
    }

    pub fn new_entity(&mut self) -> EntityView {
        let entity = self.entity_index.new_id();
        let root_archetype = self.archetypes.get_mut(self.root_archetype).unwrap();
        //let row = root_archetype.append(entity);

        // TODO: properly set row.
        self.entity_index.set_location(entity, EntityLocation{arch: self.root_archetype, row: 0});

        EntityView::new(self, entity)
    }

    pub fn new_named_entity(&mut self, name: impl Into<Rc<str>>) -> EcsResult<EntityView> {
        let name = name.into();
    
        if let Some(&id) = self.named_ids.get(&name) {
            return Err(EcsError::EntityCreate(EntityCreateError::NameInUse(id, name)));
        }
        
        let entity = self.entity_index.new_id();
        self.named_ids.insert(name, entity);
        
        //todo!("add to root archetype");

        Ok(EntityView::new(self, entity))
    }

    pub fn add_id(&mut self, entity: Entity, id: Id) -> EcsResult<()> {
        let location = self.entity_index.get_location(entity)?;
        let src_arch = location.arch;
        let dst_arch = archetype_traverse_add(self,src_arch, id);

        if src_arch != dst_arch {
            //move_entity(self, entity, src_arch, dst_arch)
        }

        Ok(())
    }

    pub fn has<T: ComponentValue>(&self, entity: Entity) -> bool {
        let id = self.type_ids.get_id::<T>().unwrap();
        self.has_id(entity, id)
    }
 
    pub fn has_id(&self, entity: Entity, id: Id) -> bool {
        let Ok(location) = self.entity_index.get_location(entity) else { return false };
        let component = &self.component_index[&id];
        component.archetypes.contains_key(&location.arch)
    }

    /// Add the component value to the entity.
    /// 
    /// Adds the component if not originally present..
    pub fn set<T: ComponentValue>(&mut self, entity: Entity, val: T) -> EcsResult<()> {
        const_assert!(|T| std::mem::size_of::<T>() != 0, "attempted to set value on a ZST");

        let id = self.type_ids.get_id::<T>().unwrap();
        self.set_id(entity, id, val)
    }

    fn set_id<T: ComponentValue>(&mut self, entity: Entity, id: Id, val: T) -> EcsResult<()> {

        Ok(())
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