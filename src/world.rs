use std::{any::TypeId, collections::HashMap, mem::ManuallyDrop, ops::{Deref, DerefMut}, ptr::NonNull, rc::Rc};
use const_assert::const_assert;
use crate::{
    component::{
        ComponentBuilder, ComponentRecord, ComponentValue, 
        ComponentView, TypedComponentBuilder, TypedComponentView
    }, component_index::ensure_component, entity::Entity, entity_index::EntityIndex, entity_view::EntityView, error::{component_not_registered_err, EcsError, EcsResult}, graph::archetype_traverse_add, id::Id, pointer::OwningPtr, storage::{
        archetype::{move_entity, move_entity_to_root},
        archetype_index::{ArchetypeBuilder, ArchetypeId, ArchetypeIndex},
    }, type_info::{Type, TypeInfo, TypeMap}, world_utils::ensure_component_ptr
};

pub struct World {
    pub(crate) entity_index: EntityIndex,
    pub(crate) components: HashMap<Id, ComponentRecord>,
    pub(crate) type_infos: HashMap<Id, Rc<TypeInfo>>,
    pub(crate) archetypes: ArchetypeIndex,
    pub(crate) archetype_map: HashMap<Type, ArchetypeId>,
    pub(crate) root_arch: ArchetypeId,
    pub(crate) type_ids: TypeMap,
}

impl World {
    pub fn new() -> Self {
        let mut world = Self {
            entity_index: EntityIndex::new(),
            components: HashMap::new(),
            type_infos: HashMap::new(),
            archetypes: ArchetypeIndex::with_capacity(100),
            archetype_map: HashMap::new(),
            root_arch: ArchetypeId::null(),
            type_ids: TypeMap::new(),
        };

        let builder = ArchetypeBuilder::new(&mut world, vec![].into());
        world.root_arch = builder.build();

        world
    }

    /// Gets the registered component or returns a builder to create one from type.
    pub fn component_t<C: ComponentValue>(&mut self) -> Result<TypedComponentView<C>, TypedComponentBuilder<C>> {
        match self.type_ids.get_t::<C>() {
            Some(id) => Ok(TypedComponentView::new(self, id)),
            None => Err(TypedComponentBuilder::new()),
        }
    }

    /// Gets the registered component or returns a builder to create one from `id`.
    pub fn component(&mut self, id: Id) -> Result<ComponentView, ComponentBuilder> {
        match self.components.get(&id) {
            Some(_) => Ok(ComponentView::new(self, id)),
            None => Err(ComponentBuilder::new(Some(id))),
        }
    }

    /// Returns a builder to create a new component.
    #[inline]
    pub fn new_component(&self) -> ComponentBuilder {
        ComponentBuilder::new(None)
    }

    pub fn new_entity(&mut self) -> EntityView {
        let entity = self.entity_index.new_id();
        move_entity_to_root(self, entity);
        EntityView::new(self, entity)
    }
    
    /// Add the `id` as tag to entity.
    /// 
    /// No side effect if the entity already contains the id.
    pub fn add(&mut self, entity: Entity, id: Id) -> EcsResult<()> {
        if self.type_infos.contains_key(&id) {
            return Err(EcsError::Component("can't use `add` for non-ZST, use `set` instead."))
        }

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

    /// Add the type as tag to entity.
    /// No side effect if entity already has tag.
    /// 
    /// Component must be registered.
    /// 
    /// Compilation fails if component is not ZST.
    pub fn add_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<()> {
        const_assert!(|C| std::mem::size_of::<C>() == 0, "can't use add_t for non-ZST, use set_t instead.");

        match self.type_ids.get_t::<C>() {
            Some(id) => self.add(entity, id),
            None => Err(EcsError::Component("component type not registered.")),
        }
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Id) -> bool {
        let Ok(location) = self.entity_index.get_location(entity) else { return false };
        let Some(component) = self.components.get(&id) else { return false;};
        component.archetypes.contains_key(&location.arch)
    }

    /// Checks if entity has the component.
    /// 
    /// Returns `false` if the type is not registered 
    /// or the entity does not have the type.
    pub fn has_t<C: ComponentValue>(&self, entity: Entity) -> bool {
        match self.type_ids.get_t::<C>() {
            Some(id) => self.has(entity, id),
            None => false,
        }
    }

    pub fn set(&mut self, entity: Entity, id: Id, value: NonNull<u8>) -> EcsResult<()> {
        Ok(())
    }

    pub fn set_t<C: ComponentValue>(&mut self, entity: Entity, value: C) -> EcsResult<()> {
        const_assert!(|C| size_of::<C>() != 0, "can't use set_t for ZST, use add_t instead");
        
        match self.type_ids.get_t::<C>() {
            Some(id) => {
                let mut md = ManuallyDrop::new(value);
                self.set(entity, id, NonNull::from(&mut(*md)).cast())
            },
            None => component_not_registered_err(),
        }
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