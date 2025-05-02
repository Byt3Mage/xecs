use crate::{
    entity::Entity,
    flags::ComponentFlags,
    storage::{
        Storage, StorageType,
        sparse_storage::{ComponentSparseSet, TagSparseSet},
    },
    type_impl::TypeImpl,
    type_info::{TypeHooks, TypeHooksBuilder, TypeInfo, TypeName},
    world::World,
};
use const_assert::const_assert;
use simple_ternary::tnr;
use std::{collections::HashMap, fmt::Debug, marker::PhantomData};

pub trait ComponentValue: TypeImpl {}
impl<T: TypeImpl> ComponentValue for T {}

/// Component location info within an [table](crate::storage::table::table).
pub(crate) struct TableRecord {
    /// First index of id within the table's [Type](crate::type_info::Type).
    pub id_index: usize,
    /// [Column](crate::storage::Column) index where the id appears.
    /// Defaults to -1 if the id is a tag.
    pub column_index: isize,
}

pub struct ComponentRecord {
    pub(crate) id: Entity,
    pub(crate) flags: ComponentFlags,
    pub(crate) storage: Storage,
    pub(crate) with_ids: Box<[Entity]>,
    pub(crate) hooks: Option<TypeHooks>,
    pub(crate) type_info: Option<&'static TypeInfo>,
}

/// Typed component id
///
/// Guarantees that the id matches the component type.
pub struct Component<C: ComponentValue> {
    id: Entity,
    type_info: &'static TypeInfo,
    _marker: PhantomData<C>,
}

impl<C: ComponentValue> Component<C> {
    /// Creates a new component with the given id and world.
    /// Returns None if the id is not a component or if the associated data type does not match.
    pub fn new(world: &World, id: Entity) -> Option<Self> {
        match world.components.get(&id) {
            Some(ComponentRecord {
                type_info: Some(ti),
                ..
            }) if ti.is::<C>() => Some(Self {
                id,
                type_info: ti,
                _marker: PhantomData,
            }),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn id(&self) -> Entity {
        self.id
    }
}

/// Untyped component id.
///
/// Guarantees that id does not have associated data.
pub struct Tag(Entity);

impl Tag {
    /// Creates a tag wrapper from the given id and world.
    /// Returns None if the id is not a component of it it has associated data.
    pub fn new(world: &World, id: Entity) -> Option<Self> {
        match world.components.get(&id) {
            Some(ComponentRecord {
                type_info: None, ..
            }) => Some(Self(id)),
            _ => None,
        }
    }

    /// Gets the id of the tag.
    #[inline(always)]
    pub fn id(&self) -> Entity {
        self.0
    }
}

pub struct TagBuilder {
    id: Entity,
    name: Option<TypeName>,
    flags: ComponentFlags,
    with_ids: Vec<Entity>,
    storage_type: StorageType,
}

impl TagBuilder {
    pub(crate) fn new(id: Entity) -> Self {
        Self {
            id,
            name: None,
            flags: ComponentFlags::empty(),
            with_ids: vec![],
            storage_type: StorageType::Tables,
        }
    }

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn storage(mut self, storage: StorageType) -> Self {
        self.storage_type = storage;
        self
    }

    pub fn with_id(mut self, id: Entity) -> Self {
        self.with_ids.push(id);
        self
    }

    pub fn with_ids(mut self, ids: impl IntoIterator<Item = Entity>) -> Self {
        self.with_ids.extend(ids);
        self
    }

    pub fn with_flag(mut self, flag: ComponentFlags) -> Self {
        self.flags.insert(flag);
        self
    }

    pub fn set_flags(mut self, flags: ComponentFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn clear_flag(mut self, flag: ComponentFlags) -> Self {
        self.flags.remove(flag);
        self
    }

    pub fn build(self, world: &mut World) -> Entity {
        let id = if self.id.is_null() {
            world.new_entity()
        } else {
            debug_assert!(
                !world.components.contains(&self.id),
                "component already exists"
            );
            self.id
        };

        let storage = match self.storage_type {
            StorageType::Tables => Storage::Tables(HashMap::new()),
            StorageType::SparseSet => Storage::SparseTag(TagSparseSet::new()),
            StorageType::PagedSparseSet(_) => todo!(),
        };

        let cr = ComponentRecord {
            id,
            flags: self.flags,
            storage,
            with_ids: self.with_ids.into_boxed_slice(),
            hooks: None,
            type_info: None,
        };

        world.components.insert(id, cr);

        id
    }
}

impl Debug for TagBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentBuilder")
            .field("id", &self.id)
            .finish()
    }
}

pub struct ComponentBuilder<C> {
    id: Entity,
    name: Option<TypeName>,
    hooks: TypeHooksBuilder<C>,
    flags: ComponentFlags,
    with_ids: Vec<Entity>,
    storage_type: StorageType,
}

impl<C: ComponentValue> ComponentBuilder<C> {
    pub(crate) fn new(id: Entity) -> Self {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't use ComponentBuilder for ZST, use TagBuilder instead"
        );

        Self {
            id,
            name: None,
            hooks: TypeHooksBuilder::new(),
            flags: ComponentFlags::empty(),
            with_ids: vec![],
            storage_type: StorageType::Tables,
        }
    }

    pub fn new_named(id: Entity, name: impl Into<TypeName>) -> Self {
        Self {
            id,
            hooks: TypeHooksBuilder::new(),
            name: Some(name.into()),
            flags: ComponentFlags::empty(),
            with_ids: vec![],
            storage_type: StorageType::Tables,
        }
    }

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn storage(mut self, storage_type: StorageType) -> Self {
        self.storage_type = storage_type;
        self
    }

    pub fn add_flags(mut self, flags: ComponentFlags) -> Self {
        self.flags.insert(flags);
        self
    }

    pub fn set_flags(mut self, flags: ComponentFlags) -> Self {
        self.flags = flags;
        self
    }

    pub fn clear_flag(mut self, flag: ComponentFlags) -> Self {
        self.flags.remove(flag);
        self
    }

    pub fn default(mut self, f: fn() -> C) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't set default hook for ZST");
        self.hooks = self.hooks.with_default(f);
        self
    }

    pub fn clone(mut self, f: fn(&C) -> C) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't set clone hook for ZST");
        self.hooks = self.hooks.with_clone(f);
        self
    }

    pub fn on_set<F>(mut self, f: F) -> Self
    where
        F: FnMut(Entity, &mut C) + 'static,
    {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_set hook for ZST");
        self.hooks = self.hooks.with_set(f);
        self
    }

    pub fn on_remove<F>(mut self, f: F) -> Self
    where
        F: FnMut(Entity, &mut C) + 'static,
    {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_remove hook for ZST");
        self.hooks = self.hooks.with_remove(f);
        self
    }

    pub fn build(self, world: &mut World) -> Entity {
        let id = if self.id.is_null() {
            let entity = world.new_entity();
            //TypeImpl::<C>::register(world, entity);
            entity
        } else {
            debug_assert!(
                !world.components.contains(&self.id),
                "component already exists"
            );
            self.id
        };

        let type_info = TypeInfo::of::<C>();

        let storage = if const { size_of::<C>() == 0 } {
            match self.storage_type {
                StorageType::Tables => Storage::Tables(HashMap::new()),
                StorageType::SparseSet => Storage::SparseTag(TagSparseSet::new()),
                StorageType::PagedSparseSet(_) => todo!(),
            }
        } else {
            match self.storage_type {
                StorageType::Tables => Storage::Tables(HashMap::new()),
                StorageType::SparseSet => Storage::SparseData(ComponentSparseSet::new(type_info)),
                StorageType::PagedSparseSet(_) => todo!(),
            }
        };

        let cr = ComponentRecord {
            id,
            flags: self.flags,
            storage,
            with_ids: self.with_ids.into_boxed_slice(),
            hooks: Some(self.hooks.build()),
            type_info: Some(type_info),
        };

        world.components.insert(id, cr);

        id
    }
}
