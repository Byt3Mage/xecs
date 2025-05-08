use crate::{
    entity::Entity,
    flags::ComponentFlags,
    storage::{
        Storage, StorageType,
        sparse_storage::{ComponentSparseSet, TagSparseSet},
    },
    types::type_info::{TypeHooksBuilder, TypeInfo, TypeName},
};
use std::{collections::HashMap, rc::Rc};

pub trait Component: 'static {}
impl<T: 'static> Component for T {}

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
    pub(crate) type_info: Option<Rc<TypeInfo>>,
    pub(crate) storage: Storage,
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

    pub fn with_type<C: Component>(mut self, type_hooks: TypeHooksBuilder<C>) -> Self {
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
                        Storage::SparseData(ComponentSparseSet::new(id, Rc::clone(&type_info)))
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

impl<C: Component> ComponentDesc<C> {
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
                        Storage::SparseData(ComponentSparseSet::new(id, Rc::clone(&type_info)))
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
