use crate::{
    component::{ComponentBuilder, ComponentRecord, ComponentValue, TypedComponentBuilder},
    entity::{Entity, HI_COMPONENT_ID},
    entity_index::EntityIndex,
    error::{EcsResult, MissingComponent, UnregisteredComponent, UnregisteredType},
    storage::{
        Storage,
        table::move_entity_to_root,
        table_index::{TableId, TableIndex},
    },
    type_id::TypeImpl,
    type_info::TypeIndex,
};
use const_assert::const_assert;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
};

macro_rules! get_component {
    ($world: expr, $id: expr) => {{
        if $id < $crate::entity::HI_COMPONENT_ID {
            Ok(&$world.components_lo[$id.id() as usize])
        } else {
            $world
                .components_hi
                .get(&$id)
                .ok_or(UnregisteredComponent($id))
        }
    }};

    (mut, $world: expr, $id: expr) => {{
        if $id < $crate::entity::HI_COMPONENT_ID {
            Ok(&mut $world.components_lo[$id.id() as usize])
        } else {
            $world
                .components_hi
                .get_mut(&$id)
                .ok_or(UnregisteredComponent($id))
        }
    }};
}
pub struct World {
    pub entity_index: EntityIndex,
    pub(crate) type_index: TypeIndex,
    pub(crate) table_index: TableIndex,
    pub(crate) root_table: TableId,
    pub(crate) type_ids: Vec<Entity>,
    pub(crate) components_lo: Vec<ComponentRecord>,
    pub(crate) components_hi: HashMap<Entity, ComponentRecord>,
}

impl World {
    pub fn new() -> Self {
        // TODO: world initialization
        Self {
            entity_index: EntityIndex::new(),
            type_index: TypeIndex::new(),
            table_index: TableIndex::new(),
            root_table: TableId::NULL,
            type_ids: Vec::new(),
            components_lo: Vec::new(),
            components_hi: HashMap::new(),
        }
    }

    /// Gets the entity for the component type.
    ///
    /// # Panics
    /// Panics if the component type is not registered.
    #[inline(always)]
    pub fn id_t<C: ComponentValue>(&mut self) -> Result<Entity, UnregisteredType> {
        TypeImpl::<C>::id(self)
    }

    /// Gets a builder for registering the component or returns the id if already registered.
    #[inline(always)]
    pub fn component_t<C: ComponentValue>(&mut self) -> Result<TypedComponentBuilder<C>, Entity> {
        match TypeImpl::<C>::id(self) {
            Ok(id) => Err(id),
            Err(_) => Ok(TypedComponentBuilder::new()),
        }
    }

    /// Gets the registered component or returns a builder to create one from `id`.
    pub fn component(&mut self, id: Entity) -> Option<ComponentBuilder> {
        match get_component!(self, id) {
            Ok(_) => None,
            Err(_) => Some(ComponentBuilder::new(id)),
        }
    }

    /// Returns a builder to create a new component.
    #[inline]
    pub fn new_component(&self) -> ComponentBuilder {
        ComponentBuilder::new(Entity::NULL)
    }

    pub fn new_entity(&mut self) -> Entity {
        let entity = self.entity_index.new_id();
        move_entity_to_root(self, entity);
        entity
    }

    /// Add the `id` as tag to entity.
    ///
    /// No side effect if the entity already contains the id.
    pub fn add(&mut self, entity: Entity, id: Entity) -> EcsResult<()> {
        todo!()
    }

    /// Add the type as tag to entity.
    /// No side effect if entity already has tag.
    ///
    /// Component must be registered.
    ///
    /// Compilation fails if component is not ZST.
    pub fn add_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<()> {
        const_assert!(
            |C| std::mem::size_of::<C>() == 0,
            "can't use add_t for component, use set_t instead."
        );

        self.add(entity, TypeImpl::<C>::id(self)?)
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Entity) -> EcsResult<bool> {
        let cr = get_component!(self, id)?;
        let r = self.entity_index.get_record(entity)?;

        match &cr.storage {
            Storage::SparseTag(set) => Ok(set.has(entity)),
            Storage::SparseData(set) => Ok(set.has(entity)),
            Storage::Tables(tables) => {
                let table = &self.table_index[r.table];

                if id < HI_COMPONENT_ID {
                    if table.component_map_lo[id.as_usize()] >= 0 {
                        return Ok(true);
                    }
                }

                Ok(tables.contains_key(&r.table))
            }
        }
    }

    /// Checks if entity has the component.
    ///
    /// Returns `false` if the type is not registered
    /// or the entity does not have the type.
    pub fn has_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<bool> {
        self.has(entity, TypeImpl::<C>::id(self)?)
    }

    #[inline]
    pub fn set<C: ComponentValue>(
        &mut self,
        entity: Entity,
        id: Entity,
        value: C,
    ) -> EcsResult<()> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use set for tag, did you want to add?"
        );
        todo!()
    }

    pub fn set_t<C: ComponentValue>(&mut self, entity: Entity, value: C) -> EcsResult<()> {
        self.set(entity, TypeImpl::<C>::id(self)?, value)
    }

    #[inline]
    pub fn get<C: ComponentValue>(&self, entity: Entity, id: Entity) -> EcsResult<&C> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use get for tag, did you want to check with has?"
        );

        let cr = get_component!(self, id)?;
        let r = self.entity_index.get_record(entity)?;
        let comp = match &cr.storage {
            Storage::SparseTag(_) => None,
            Storage::SparseData(set) => {
                // SAFETY: TODO: type checking.
                unsafe { set.get::<C>(entity) }
            }
            Storage::Tables(_) => {
                let t = &self.table_index[r.table];
                // SAFETY: valid entity must have valid row
                unsafe { t.get::<C>(r.row, id) }
            }
        };

        comp.ok_or_else(|| MissingComponent(id, entity).into())
    }

    pub fn get_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<&C> {
        self.get(entity, TypeImpl::<C>::id(self)?)
    }

    #[inline]
    pub fn get_mut<C: ComponentValue>(&mut self, entity: Entity, id: Entity) -> EcsResult<&mut C> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use get for tag, did you want to check with has?"
        );

        // TODO: type checking.

        let cr = get_component!(mut, self, id)?;
        let r = self.entity_index.get_record(entity)?;
        let comp = match &mut cr.storage {
            Storage::SparseTag(_) => None,
            Storage::SparseData(set) => unsafe { set.get_mut::<C>(entity) },
            Storage::Tables(_) => {
                let t = &mut self.table_index[r.table];
                // SAFETY: valid entity must have valid row
                unsafe { t.get_mut::<C>(r.row, id) }
            }
        };

        comp.ok_or_else(|| MissingComponent(id, entity).into())
    }

    pub fn get_mut_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<&mut C> {
        self.get_mut(entity, TypeImpl::<C>::id(self)?)
    }
}

pub struct WorldRef<'world> {
    world: &'world mut World,
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

impl<'world> From<&'world mut World> for WorldRef<'world> {
    fn from(value: &'world mut World) -> Self {
        Self { world: value }
    }
}
