use crate::{
    entity::Entity,
    flags::ComponentFlags,
    storage::{
        Storage, StorageType,
        sparse_storage::{ComponentSparseSet, TagSparseSet},
    },
    types::type_info::{TypeHooksBuilder, TypeInfo, TypeName},
    world::World,
};
use const_assert::const_assert;
use std::{collections::HashMap, marker::PhantomData, rc::Rc};

pub trait ComponentValue: 'static {}
impl<T: 'static> ComponentValue for T {}

/// Component location info within an [table](crate::storage::table::table).
pub(crate) struct TableRecord {
    /// First index of id within the table's [Type](crate::type_info::Type).
    pub id_index: usize,
    /// [Column](crate::storage::Column) index where the id appears.
    /// Defaults to -1 if the id is a tag.
    pub column_index: isize,
}

pub(crate) struct ComponentRecord {
    pub(crate) id: Entity,
    pub(crate) flags: ComponentFlags,
    pub(crate) type_info: Option<Rc<TypeInfo>>,
    pub(crate) storage: Storage,
}

/// Typesafe wrapper around a component id.
/// Guarantees that the id holds the correct data type.
pub struct Component<C: ComponentValue> {
    pub(crate) id: Entity,
    pub(crate) _marker: PhantomData<C>,
}

impl<C: ComponentValue> core::marker::Copy for Component<C> {}
impl<C: ComponentValue> core::clone::Clone for Component<C> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: PhantomData,
        }
    }
}

impl<C: ComponentValue> core::cmp::Eq for Component<C> {}
impl<C: ComponentValue> core::cmp::PartialEq for Component<C> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl<C: ComponentValue> core::hash::Hash for Component<C> {
    fn hash<H: core::hash::Hasher>(&self, ra_expand_state: &mut H) {
        self.id.hash(ra_expand_state);
    }
}

impl<C: ComponentValue> From<Component<C>> for Entity {
    fn from(value: Component<C>) -> Self {
        value.id
    }
}

impl<C: ComponentValue> Component<C> {
    /// Create a wrapper with the correct associated data type.
    /// Returns `None` if the id is not a component
    /// or there is a type mismatch.
    pub fn new(world: &World, id: Entity) -> Option<Self> {
        match world.components.get(&id) {
            Some(ComponentRecord {
                type_info: Some(ti),
                ..
            }) if ti.is::<C>() => Some(Self {
                id,
                _marker: PhantomData,
            }),
            _ => None,
        }
    }

    #[inline]
    pub fn id(&self) -> Entity {
        self.id
    }
}

/// Typesafe wrapper around a component id.
/// Guarantees that the id does not have associated data.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tag(pub(crate) Entity);

impl Tag {
    /// Create a wrapper for the id.
    /// Returns `None` if the id is not a component
    /// or if it contains associated data.
    pub fn new(world: &World, id: Entity) -> Option<Self> {
        match world.components.get(&id) {
            Some(cr) if cr.type_info.is_none() => Some(Self(id)),
            _ => None,
        }
    }

    #[inline]
    pub fn id(&self) -> Entity {
        self.0
    }
}

impl From<Tag> for Entity {
    fn from(value: Tag) -> Self {
        value.0
    }
}

/// Typesafe wrapper around a component id.
/// Guarantees that the type is associated with an id.
pub struct TypedEntity<C: ComponentValue> {
    pub(crate) id: Entity,
    pub(crate) _marker: PhantomData<C>,
}

impl<C: ComponentValue> core::marker::Copy for TypedEntity<C> {}
impl<C: ComponentValue> core::clone::Clone for TypedEntity<C> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: PhantomData,
        }
    }
}

impl<C: ComponentValue> core::cmp::Eq for TypedEntity<C> {}
impl<C: ComponentValue> core::cmp::PartialEq for TypedEntity<C> {
    fn eq(&self, other: &Self) -> bool {
        self.id.eq(&other.id)
    }
}

impl<C: ComponentValue> core::hash::Hash for TypedEntity<C> {
    fn hash<H: core::hash::Hasher>(&self, ra_expand_state: &mut H) {
        self.id.hash(ra_expand_state);
    }
}

impl<C: ComponentValue> TypedEntity<C> {
    /// Create a wrapper for the type.
    /// Returns `None` if the type does not have associated id.
    pub fn new(world: &World) -> Option<Self> {
        world.type_map.get::<C>().map(|&id| Self {
            id,
            _marker: PhantomData,
        })
    }

    #[inline]
    pub fn id(&self) -> Entity {
        self.id
    }
}

impl<C: ComponentValue> From<TypedEntity<C>> for Entity {
    fn from(value: TypedEntity<C>) -> Self {
        value.id
    }
}

impl<C: ComponentValue> From<TypedEntity<C>> for Tag {
    fn from(value: TypedEntity<C>) -> Self {
        const_assert!(
            |C| size_of::<C>() == 0,
            "can't convert non-ZST TypedEntity to Tag"
        );
        Tag(value.id)
    }
}

impl<C: ComponentValue> From<TypedEntity<C>> for Component<C> {
    fn from(value: TypedEntity<C>) -> Self {
        const_assert!(
            |C| size_of::<C>() != 0,
            "can't convert ZST TypedEntity to Component"
        );
        Component {
            id: value.id,
            _marker: PhantomData,
        }
    }
}

pub struct UntypedComponentDesc {
    name: Option<TypeName>,
    type_info: Option<TypeInfo>,
    flags: ComponentFlags,
    storage_type: StorageType,
}

impl UntypedComponentDesc {
    pub fn new() -> Self {
        Self {
            name: None,
            type_info: None,
            flags: ComponentFlags::empty(),
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

    pub fn with_type<C: ComponentValue>(mut self, type_hooks: TypeHooksBuilder<C>) -> Self {
        self.type_info = Some(TypeInfo::new(type_hooks));
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

    pub(crate) fn build(self, id: Entity) -> ComponentRecord {
        let (type_info, storage) = match self.type_info {
            Some(type_info) => {
                let type_info = Rc::new(type_info);
                let storage = match self.storage_type {
                    StorageType::Tables => Storage::Tables(HashMap::new()),
                    StorageType::SparseSet => {
                        Storage::SparseData(ComponentSparseSet::new(Rc::clone(&type_info)))
                    }
                    StorageType::PagedSparseSet(_) => todo!(),
                };

                (Some(type_info), storage)
            }
            None => {
                let storage = match self.storage_type {
                    StorageType::Tables => Storage::Tables(HashMap::new()),
                    StorageType::SparseSet => Storage::SparseTag(TagSparseSet::new()),
                    StorageType::PagedSparseSet(_) => todo!(),
                };

                (None, storage)
            }
        };

        ComponentRecord {
            id,
            flags: self.flags,
            type_info,
            storage,
        }
    }
}

pub struct ComponentDesc<C> {
    name: Option<TypeName>,
    hooks: Option<TypeHooksBuilder<C>>,
    flags: ComponentFlags,
    storage_type: StorageType,
}

impl<C: ComponentValue> ComponentDesc<C> {
    pub fn new() -> Self {
        let hooks = const {
            if size_of::<C>() != 0 {
                Some(TypeHooksBuilder::new())
            } else {
                None
            }
        };

        Self {
            name: None,
            hooks,
            flags: ComponentFlags::empty(),
            storage_type: StorageType::Tables,
        }
    }

    #[inline]
    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[inline]
    pub fn storage(mut self, storage_type: StorageType) -> Self {
        self.storage_type = storage_type;
        self
    }

    #[inline]
    pub fn add_flags(mut self, flags: ComponentFlags) -> Self {
        self.flags.insert(flags);
        self
    }

    #[inline]
    pub fn set_flags(mut self, flags: ComponentFlags) -> Self {
        self.flags = flags;
        self
    }

    #[inline]
    pub fn clear_flags(mut self, flags: ComponentFlags) -> Self {
        self.flags.remove(flags);
        self
    }

    #[inline]
    pub fn default(mut self, f: fn() -> C) -> Self {
        self.hooks = self.hooks.map(|b| b.with_default(f));
        self
    }

    #[inline]
    pub fn clone(mut self, f: fn(&C) -> C) -> Self {
        self.hooks = self.hooks.map(|b| b.with_clone(f));
        self
    }

    #[inline]
    pub fn on_set(mut self, f: impl FnMut(Entity, &mut C) + 'static) -> Self {
        self.hooks = self.hooks.map(|b| b.on_set(f));
        self
    }

    #[inline]
    pub fn on_remove(mut self, f: impl FnMut(Entity, &mut C) + 'static) -> Self {
        self.hooks = self.hooks.map(|b| b.on_remove(f));
        self
    }

    pub(crate) fn build(self, id: Entity) -> ComponentRecord {
        let (type_info, storage) = match self.hooks {
            Some(hooks) => {
                let type_info = Rc::new(TypeInfo::new(hooks));
                let storage = match self.storage_type {
                    StorageType::Tables => Storage::Tables(HashMap::new()),
                    StorageType::SparseSet => {
                        Storage::SparseData(ComponentSparseSet::new(Rc::clone(&type_info)))
                    }
                    StorageType::PagedSparseSet(_) => todo!(),
                };

                (Some(type_info), storage)
            }
            None => {
                let storage = match self.storage_type {
                    StorageType::Tables => Storage::Tables(HashMap::new()),
                    StorageType::SparseSet => Storage::SparseTag(TagSparseSet::new()),
                    StorageType::PagedSparseSet(_) => todo!(),
                };

                (None, storage)
            }
        };

        ComponentRecord {
            id,
            flags: self.flags,
            type_info,
            storage,
        }
    }
}
