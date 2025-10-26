use crate::{
    flags::ComponentFlags,
    id::Id,
    storage::{
        Storage, StorageType,
        sparse::{SparseData, SparseTag},
    },
    type_info::{TypeHooksBuilder, TypeInfo, TypeName},
    type_traits::{Component, DataComponent},
    world::World,
};
use std::{collections::HashMap, rc::Rc};

/// Component location in a [Table](crate::storage::table::Table).
pub(crate) struct ComponentLocation {
    /// Index of id in the table's [IdList](crate::types::IdList).
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

    fn build(mut self, world: &mut World, id: Id) {
        debug_assert!(id.is_id(), "attempted to build pair as entity");

        self.flags.insert(ComponentFlags::IS_TAG);

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

pub struct ComponentBuilder<T: DataComponent> {
    name: Option<TypeName>,
    hooks: TypeHooksBuilder<T>,
    flags: ComponentFlags,
    storage_type: StorageType,
}

impl<T: Component + DataComponent> ComponentBuilder<T> {
    pub fn new() -> Self {
        Self {
            name: None,
            hooks: TypeHooksBuilder::new(),
            flags: ComponentFlags::empty(),
            storage_type: T::STORAGE,
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
    pub fn on_set(mut self, f: impl FnMut(Id, &mut T) + 'static) -> Self {
        self.hooks = self.hooks.on_set(f);
        self
    }

    #[inline]
    pub fn on_remove(mut self, f: impl FnMut(Id, &mut T) + 'static) -> Self {
        self.hooks = self.hooks.on_remove(f);
        self
    }

    pub(crate) fn build(mut self, world: &mut World, id: Id) {
        debug_assert!(id.is_id(), "attempted to build pair as entity");

        let type_info = Rc::new(TypeInfo::of::<T>(self.hooks));

        let storage = match self.storage_type {
            StorageType::Tables => Storage::Tables(HashMap::new()),
            StorageType::Sparse => Storage::SparseData(SparseData::new(id, Rc::clone(&type_info))),
        };

        self.flags.remove(ComponentFlags::IS_TAG);

        world.components.insert(
            id,
            ComponentInfo {
                id,
                flags: self.flags,
                type_info: Some(type_info),
                storage,
            },
        );
    }
}

/// Ensures that a component exists for this id.
///
/// This function creates the component as a tag if it didn't exist.
pub(crate) fn ensure_component(world: &mut World, comp: Id) {
    if !world.components.contains(comp) {
        if comp.is_pair() {
            build_pair(world, comp);
        } else {
            // We build component as tag since we don't have type info.
            TagBuilder::new().build(world, comp);
        }
    }
}

pub(crate) fn build_pair(world: &mut World, id: Id) {
    debug_assert!(id.is_pair(), "attemped to build entity as pair");

    let rel = world.id_manager.get_current(id.pair_rel()).unwrap();
    let tgt = world.id_manager.get_current(id.pair_tgt()).unwrap();

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
                cr_t.type_info.as_ref().map(Rc::clone)
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

pub(crate) mod private {
    pub struct Passkey;
}

#[doc(hidden)]
pub trait ComponentDescriptor {
    fn build(self, world: &mut World, id: Id, _: private::Passkey);
}

impl ComponentDescriptor for TagBuilder {
    #[inline(always)]
    fn build(self, world: &mut World, id: Id, _: private::Passkey) {
        self.build(world, id);
    }
}

impl<T: Component + DataComponent> ComponentDescriptor for ComponentBuilder<T> {
    fn build(self, world: &mut World, id: Id, _: private::Passkey) {
        self.build(world, id);
    }
}
