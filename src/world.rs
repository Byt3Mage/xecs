use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    ptr::NonNull,
};

use const_assert::const_assert;

use crate::{
    component::{
        ComponentBuilder, ComponentRecord, ComponentValue, ComponentView, TypedComponentBuilder,
    },
    component_index::ComponentIndex,
    entity::Entity,
    entity_index::EntityIndex,
    error::{EcsError, EcsResult, component_not_registered_err},
    graph::table_traverse_add,
    id::{HI_COMPONENT_ID, Id, pair},
    storage::{
        table::{Table, move_entity, move_entity_to_root},
        table_index::TableIndex,
    },
    type_info::{Type, TypeIndex, TypeMap},
    world_utils::{get_component_data, get_component_data_mut, set_component_value},
};

pub struct World {
    pub entity_index: EntityIndex,
    pub(crate) components: ComponentIndex,
    pub(crate) type_index: TypeIndex,
    pub(crate) tables: TableIndex,
    pub(crate) table_map: HashMap<Type, NonNull<Table>>,
    pub(crate) root_table: NonNull<Table>,
    pub(crate) type_ids: TypeMap,
    pub(crate) components_lo: Vec<ComponentRecord>,
    pub(crate) components_hi: HashMap<Id, ComponentRecord>,
}

impl World {
    pub fn new() -> Self {
        Self {
            entity_index: EntityIndex::new(),
            components: ComponentIndex::new(),
            type_index: TypeIndex::new(),
            tables: TableIndex::new(),
            table_map: HashMap::new(),
            root_table: NonNull::dangling(),
            type_ids: TypeMap::new(),
            components_lo: Vec::new(),
            components_hi: HashMap::new(),
        }
    }

    #[inline(always)]
    pub fn id_t<C: ComponentValue>(&mut self) -> Id {
        self.type_ids.get_t::<C>().unwrap_or(0)
    }

    /// Gets a builder for registering the component or returns the id if already registered.
    #[inline(always)]
    pub fn component_t<C: ComponentValue>(&mut self) -> Result<TypedComponentBuilder<C>, Id> {
        match self.type_ids.get_t::<C>() {
            None => Ok(TypedComponentBuilder::new()),
            Some(id) => Err(id),
        }
    }

    /// Gets the registered component or returns a builder to create one from `id`.
    pub fn component(&mut self, id: Id) -> Result<ComponentView, ComponentBuilder> {
        match self.components.get(id) {
            Some(_) => Ok(ComponentView::new(self, id)),
            None => Err(ComponentBuilder::new(Some(id))),
        }
    }

    /// Returns a builder to create a new component.
    #[inline]
    pub fn new_component(&self) -> ComponentBuilder {
        ComponentBuilder::new(None)
    }

    pub fn new_entity(&mut self) -> Entity {
        let entity = self.entity_index.new_id();
        move_entity_to_root(self, entity);
        entity
    }

    pub(crate) fn get_component(&self, id: Id) -> Option<&ComponentRecord> {
        if id < HI_COMPONENT_ID {
            self.components_lo.get(id as usize)
        } else {
            self.components_hi.get(&id)
        }
    }

    /// Add the `id` as tag to entity.
    ///
    /// No side effect if the entity already contains the id.
    pub fn add(&mut self, entity: Entity, id: Id) -> EcsResult<()> {
        if self.type_index.has_info(id) {
            return Err(EcsError::Component(
                "can't use `add` for non-ZST, use `set` instead.",
            ));
        }

        let record = self.entity_index.get_record(entity)?;
        debug_assert!(record.table.is_some());

        let row = record.row;

        // SAFETY: valid records must have a table.
        let mut src_table = unsafe { record.table.unwrap_unchecked() };
        let mut dst_table = table_traverse_add(self, src_table, id);

        if src_table != dst_table {
            // SAFETY:
            // - src_row is valid in enitity index.
            // - we just checked that src_arch and dst_arch are not the same.
            unsafe {
                move_entity(self, entity, src_table.as_mut(), row, dst_table.as_mut());
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
        const_assert!(
            |C| std::mem::size_of::<C>() == 0,
            "can't use add_t for component, use set_t instead."
        );

        match self.type_ids.get_t::<C>() {
            Some(id) => self.add(entity, id),
            None => component_not_registered_err(),
        }
    }

    #[inline]
    pub fn add_r(&mut self, entity: Entity, rel: Id, obj: Id) -> EcsResult<()> {
        self.add(entity, pair(rel, obj))
    }

    /// Checks if the entity has the component.
    pub fn has(&self, entity: Entity, id: Id) -> bool {
        let Ok(record) = self.entity_index.get_record(entity) else {
            return false;
        };
        debug_assert!(record.table.is_some(), "valid entity must have a table");

        // SAFETY: valid record must have a table
        let table = unsafe { record.table.unwrap_unchecked().as_ref() };

        if id < HI_COMPONENT_ID {
            if table.component_map_lo[id as usize] != 0 {
                return true;
            }
        }

        if let Some(cr) = self.get_component(id) {
            if cr.tables.contains_key(&table.id) {
                return true;
            }
            /*if cr.flags.contains(ComponentFlags::IS_SPARSE) {
                // TODO: check sparse
            }*/
        }

        false
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

    #[inline]
    pub fn has_p(&self, entity: Entity, rel: Id, obj: Id) -> bool {
        self.has(entity, pair(rel, obj))
    }

    #[inline]
    pub fn set<C: ComponentValue>(&mut self, entity: Entity, id: Id, value: C) -> EcsResult<()> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use set for tag, did you want to add?"
        );
        set_component_value(self, entity, id, value)
    }

    pub fn set_t<C: ComponentValue>(&mut self, entity: Entity, value: C) -> EcsResult<()> {
        match self.type_ids.get_t::<C>() {
            Some(id) => self.set(entity, id, value),
            None => component_not_registered_err(),
        }
    }

    #[inline]
    pub fn get<C: ComponentValue>(&self, entity: Entity, id: Id) -> EcsResult<&C> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use get for tag, did you want to check with has?"
        );
        get_component_data(self, entity, id)
    }

    pub fn get_t<C: ComponentValue>(&self, entity: Entity) -> EcsResult<&C> {
        match self.type_ids.get_t::<C>() {
            Some(id) => self.get(entity, id),
            None => component_not_registered_err(),
        }
    }

    #[inline]
    pub fn get_mut<C: ComponentValue>(&mut self, entity: Entity, id: Id) -> EcsResult<&mut C> {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use get for tag, did you want to check with has?"
        );
        get_component_data_mut(self, entity, id)
    }

    pub fn get_mut_t<C: ComponentValue>(&mut self, entity: Entity) -> EcsResult<&mut C> {
        match self.type_ids.get_t::<C>() {
            Some(id) => self.get_mut(entity, id),
            None => component_not_registered_err(),
        }
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
