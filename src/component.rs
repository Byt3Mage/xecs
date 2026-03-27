use ahash::AHashMap;

use crate::{
    flags::ComponentFlags,
    id::{Entity, Id, IdMap, pair::Pair},
    storage::{
        Storage, StorageType,
        sparse::{SparseData, SparseTag},
    },
    type_info::{TypeHooksBuilder, TypeIndex, TypeInfo, TypeName},
    type_traits::{ComponentType, Data},
    world::World,
};

use std::rc::Rc;

/// # Safety
/// DO NOT implement this trait directly, use #\[derive(Component)\] instead.
pub unsafe trait Component: Sized + 'static {
    type DataType: ComponentType;
    type DescType: ComponentDescriptor;
    const IS_GENERIC: bool;
    const DEFAULT_STORAGE: StorageType = StorageType::Tables;

    #[doc(hidden)]
    fn type_index() -> TypeIndex;

    #[doc(hidden)]
    fn id(world: &World) -> Option<Entity> {
        if Self::IS_GENERIC {
            world.type_map.get::<Self>().copied()
        } else {
            let idx = Self::type_index().get();
            world.type_arr.get(idx).copied().flatten()
        }
    }

    fn get_or_register_type(world: &mut World) -> Entity {
        if Self::IS_GENERIC {
            if let Some(&id) = world.type_map.get::<Self>() {
                return id;
            }
            let new_id = world.new_entity();
            world.type_map.insert::<Self>(new_id);
            new_id
        } else {
            let idx = Self::type_index().get();

            if idx >= world.type_arr.len() {
                world.type_arr.resize(idx + 1, None);
            }

            if let Some(id) = world.type_arr[idx] {
                return id;
            }

            let new_id = world.new_entity();
            world.type_arr[idx] = Some(new_id);
            new_id
        }
    }
}

/// Component location in a [Table](crate::storage::table::Table).
pub(crate) struct ComponentLocation {
    /// Index of id in the table's [Signature](crate::entity::Signature).
    pub(crate) id_idx: usize,
    /// [Column](crate::storage::Column) index where the id appears.
    /// Defaults to `None` if the id is a tag.
    pub(crate) col_idx: Option<usize>,
}

pub(crate) struct ComponentInfo {
    pub(crate) flags: ComponentFlags,
    pub(crate) type_info: Option<Rc<TypeInfo>>,
    pub(crate) storage: Storage,
}

pub struct TagBuilder {
    name: Option<TypeName>,
    flags: ComponentFlags,
    storage_type: StorageType,
}

impl TagBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            flags: ComponentFlags::empty(),
            storage_type: StorageType::default(),
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

    pub fn with_flags(mut self, flag: ComponentFlags) -> Self {
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

    fn build<T: Id>(mut self, world: &mut World, id: T) {
        self.flags.insert(ComponentFlags::IS_TAG);

        let storage = match self.storage_type {
            StorageType::Tables => Storage::Tables(AHashMap::new()),
            StorageType::Sparse => Storage::SparseTag(SparseTag::new()),
        };

        id.map_insert(
            &mut world.components,
            ComponentInfo {
                flags: self.flags,
                type_info: None,
                storage,
            },
        );
    }
}

pub struct ComponentBuilder<T: Component<DataType = Data>> {
    name: Option<TypeName>,
    flags: ComponentFlags,
    storage_type: StorageType,
    hooks: TypeHooksBuilder<T>,
}

impl<T: Component<DataType = Data>> ComponentBuilder<T> {
    pub fn new() -> Self {
        Self {
            name: None,
            hooks: TypeHooksBuilder::new(),
            flags: ComponentFlags::empty(),
            storage_type: T::DEFAULT_STORAGE,
        }
    }

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
    pub fn default(mut self, f: fn() -> T) -> Self {
        self.hooks = self.hooks.with_default(f);
        self
    }

    #[inline]
    pub fn clone(mut self, f: fn(&T) -> T) -> Self {
        self.hooks = self.hooks.with_clone(f);
        self
    }

    #[inline]
    pub fn on_set(mut self, f: impl FnMut(Entity, &mut T) + 'static) -> Self {
        self.hooks = self.hooks.on_set(f);
        self
    }

    #[inline]
    pub fn on_remove(mut self, f: impl FnMut(Entity, &mut T) + 'static) -> Self {
        self.hooks = self.hooks.on_remove(f);
        self
    }

    pub(crate) fn build(mut self, components: &mut IdMap<ComponentInfo>, id: Entity) {
        let type_info = Rc::new(TypeInfo::of::<T>(self.hooks.build()));

        self.flags.remove(ComponentFlags::IS_TAG);

        id.map_insert(
            components,
            ComponentInfo {
                flags: self.flags,
                type_info: Some(type_info.clone()),
                storage: match self.storage_type {
                    StorageType::Tables => Storage::Tables(AHashMap::new()),
                    StorageType::Sparse => Storage::SparseData(SparseData::new(type_info)),
                },
            },
        );
    }
}

/// Ensures that a component exists for this id.
///
/// This function creates the component as a tag if it didn't exist.
pub(crate) fn ensure_entity_comp(map: &mut IdMap<ComponentInfo>, comp: Entity) -> &ComponentInfo {
    if !comp.map_contains_key(map) {
        comp.map_insert(
            map,
            ComponentInfo {
                flags: ComponentFlags::IS_TAG,
                type_info: None,
                storage: Storage::Tables(AHashMap::new()),
            },
        );
    }

    comp.map_get(map).unwrap()
}

pub(crate) fn build_pair(world: &mut World, pair: Pair) {
    // ensure both relation and target are valid, alive entities.
    assert!(world.is_alive(pair.rel) && world.is_alive(pair.tgt));

    let map = &mut world.components;

    // TODO: use world default storage type
    let ci_r = ensure_entity_comp(map, pair.rel);
    let flags = ci_r.flags;
    let storage_type = ci_r.storage.get_type();

    // TODO: pair storages.

    let type_info = ci_r
        .type_info
        .clone()
        .or_else(|| pair.tgt.map_get(map)?.type_info.clone());

    let storage = match storage_type {
        StorageType::Tables => Storage::Tables(AHashMap::new()),
        StorageType::Sparse => match &type_info {
            Some(ti) => Storage::SparseData(SparseData::new(ti.clone())),
            None => Storage::SparseTag(SparseTag::new()),
        },
    };

    pair.map_insert(
        map,
        ComponentInfo {
            flags,
            type_info,
            storage,
        },
    );
}

pub(crate) mod private {
    pub struct Passkey;
}

#[doc(hidden)]
pub trait ComponentDescriptor {
    fn build(self, world: &mut World, id: Entity, _: private::Passkey);
}

impl ComponentDescriptor for TagBuilder {
    #[inline(always)]
    fn build(self, world: &mut World, id: Entity, _: private::Passkey) {
        self.build(world, id);
    }
}

impl<T: Component<DataType = Data>> ComponentDescriptor for ComponentBuilder<T> {
    fn build(self, world: &mut World, id: Entity, _: private::Passkey) {
        self.build(&mut world.components, id);
    }
}
