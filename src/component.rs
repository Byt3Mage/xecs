use crate::{
    flags::ComponentFlags,
    id::Id,
    storage::{
        Storage, StorageType,
        sparse_set::{SparseData, SparseTag},
    },
    types::type_info::{TypeHooksBuilder, TypeInfo, TypeName},
    world::World,
};
use std::{collections::HashMap, rc::Rc};

pub trait Component: 'static {}
// TODO: check if I want blanket implementation.
impl<T: 'static> Component for T {}

/// Component location in a [Table](crate::storage::table::Table).
pub(crate) struct TableRecord {
    /// First index of id in the table's [IdList](crate::types::IdList).
    pub(crate) id_idx: usize,
    /// [Column](crate::storage::Column) index where the id appears.
    /// Defaults to `None` if the id is a tag.
    pub(crate) col_idx: Option<usize>,
}

pub(crate) struct ComponentInfo {
    pub(crate) id: Id,
    pub(crate) flags: ComponentFlags,
    pub(crate) type_info: Option<Rc<TypeInfo>>,
    pub(crate) storage: Storage,
}

pub struct TagDesc {
    // TODO: component name
    name: Option<TypeName>,
    flags: ComponentFlags,
    storage_type: StorageType,
}

impl TagDesc {
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

    pub(crate) fn build(mut self, world: &mut World, id: Id) {
        debug_assert!(id.is_entity(), "attempted to build pair as entity");

        self.flags.remove(ComponentFlags::IS_TAG);

        let storage = match self.storage_type {
            StorageType::Tables => Storage::Tables(HashMap::new()),
            StorageType::Sparse => Storage::SparseTag(SparseTag::new()),
        };

        world.components.insert(
            id,
            ComponentInfo {
                id,
                flags: self.flags,
                type_info: None,
                storage,
            },
        );
    }
}

pub struct ComponentDesc<C> {
    // TODO: component name
    name: Option<TypeName>,
    hooks: TypeHooksBuilder<C>,
    flags: ComponentFlags,
    storage_type: StorageType,
}

impl<C: Component> ComponentDesc<C> {
    pub fn new() -> Self {
        Self {
            name: None,
            hooks: TypeHooksBuilder::new(),
            flags: ComponentFlags::empty(),
            storage_type: StorageType::Tables,
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
    pub fn default(mut self, f: fn() -> C) -> Self {
        self.hooks = self.hooks.with_default(f);
        self
    }

    #[inline]
    pub fn clone(mut self, f: fn(&C) -> C) -> Self {
        self.hooks = self.hooks.with_clone(f);
        self
    }

    #[inline]
    pub fn on_set(mut self, f: impl FnMut(Id, &mut C) + 'static) -> Self {
        self.hooks = self.hooks.on_set(f);
        self
    }

    #[inline]
    pub fn on_remove(mut self, f: impl FnMut(Id, &mut C) + 'static) -> Self {
        self.hooks = self.hooks.on_remove(f);
        self
    }

    pub(crate) fn build(mut self, world: &mut World, id: Id) {
        debug_assert!(id.is_entity(), "attempted to build pair as entity");

        let (type_info, storage) = match TypeInfo::of::<C>(self.hooks).map(Rc::new) {
            Some(ti) => {
                let storage = match self.storage_type {
                    StorageType::Tables => Storage::Tables(HashMap::new()),
                    StorageType::Sparse => Storage::SparseData(SparseData::new(id, Rc::clone(&ti))),
                };
                (Some(ti), storage)
            }
            None => {
                let storage = match self.storage_type {
                    StorageType::Tables => Storage::Tables(HashMap::new()),
                    StorageType::Sparse => Storage::SparseTag(SparseTag::new()),
                };
                (None, storage)
            }
        };

        self.flags.remove(ComponentFlags::IS_TAG);

        world.components.insert(
            id,
            ComponentInfo {
                id,
                flags: self.flags,
                type_info,
                storage,
            },
        );
    }
}

/// Ensures that a component exists for this id.
///
/// This function creates the component as a tag if it didn't exist.
pub(crate) fn ensure_component(world: &mut World, id: Id) {
    if !world.components.contains(id) {
        if id.is_pair() {
            build_pair(world, id);
        } else {
            // We build component as tag since we don't have type info.
            TagDesc::new().build(world, id);
        }
    }
}

pub(crate) fn build_pair(world: &mut World, id: Id) {
    debug_assert!(id.is_pair(), "attemped to build entity as pair");

    let rel = world.id_index.get_current(id.pair_rel());
    let tgt = world.id_index.get_current(id.pair_tgt());

    assert!(!rel.is_null() && !tgt.is_null());

    ensure_component(world, rel);

    let ci_r = world.components.get(rel).unwrap();
    let flags = ci_r.flags;
    let storage_type = ci_r.storage.get_type();

    // TODO: pair storages.

    let type_info = {
        match &ci_r.type_info {
            Some(ti) => Some(Rc::clone(ti)),
            None => {
                ensure_component(world, tgt);
                let cr_t = world.components.get(tgt).unwrap();
                match &cr_t.type_info {
                    Some(ti) => Some(Rc::clone(ti)),
                    None => None,
                }
            }
        }
    };

    let storage = match storage_type {
        StorageType::Tables => Storage::Tables(HashMap::new()),
        StorageType::Sparse => match &type_info {
            Some(ti) => Storage::SparseData(SparseData::new(id, Rc::clone(ti))),
            None => Storage::SparseTag(SparseTag::new()),
        },
    };

    world.components.insert(
        id,
        ComponentInfo {
            id,
            flags,
            type_info,
            storage,
        },
    );
}
