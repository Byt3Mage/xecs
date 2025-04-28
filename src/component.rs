use crate::{
    entity::Entity,
    flags::ComponentFlags,
    storage::Storage,
    type_info::{HooksBuilder, TypeInfo, TypeName},
    world::World,
};
use const_assert::const_assert;
use simple_ternary::tnr;
use std::{alloc::Layout, any::TypeId, fmt::Debug, rc::Rc};

pub trait ComponentValue: 'static {}
impl<T: 'static> ComponentValue for T {}

/// Component location info within an [table](crate::storage::table::table).
pub(crate) struct ComponentLocation {
    /// First index of id within the table's [Type](crate::type_info::Type).
    pub id_index: usize,
    /// Number of times the id occurs in the table. E.g id, (id, \*), (\*, id).
    pub id_count: usize,
    /// First [Column](crate::storage::Column) index where the id appears (if not tag).
    pub column_index: isize,
}

pub struct ComponentRecord {
    pub(crate) id: Entity,
    pub(crate) flags: ComponentFlags,
    pub(crate) type_info: Option<Rc<TypeInfo>>,
    pub(crate) storage: Storage,
}

pub struct ComponentBuilder {
    id: Entity,
    name: Option<TypeName>,
    flags: ComponentFlags,
    type_info: Option<TypeInfo>,
}

impl ComponentBuilder {
    pub(crate) fn new(id: Entity) -> Self {
        Self {
            id,
            name: None,
            flags: ComponentFlags::empty(),
            type_info: None,
        }
    }

    pub fn new_named(id: Entity, name: impl Into<TypeName>) -> Self {
        Self {
            id,
            name: Some(name.into()),
            flags: ComponentFlags::empty(),
            type_info: None,
        }
    }

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn set_type<C: ComponentValue>(mut self, hooks: HooksBuilder<C>) -> Self {
        self.type_info = Some(TypeInfo {
            id: Entity::NULL,
            layout: Layout::new::<C>(),
            hooks: hooks.build(),
            type_name: None,
            type_id: TypeId::of::<C>(),
        });

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

    pub(crate) fn build(self, world: &mut World) -> Entity {
        todo!()
    }
}

impl Debug for ComponentBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentBuilder")
            .field("id", &self.id)
            .finish()
    }
}

pub struct TypedComponentBuilder<C> {
    hooks: Option<HooksBuilder<C>>,
    name: Option<TypeName>,
    flags: ComponentFlags,
}

impl<C: ComponentValue> TypedComponentBuilder<C> {
    pub(crate) fn new() -> Self {
        Self {
            hooks: const {
                tnr! {size_of::<C>() != 0 => Some(HooksBuilder::new()) : None}
            },
            name: None,
            flags: ComponentFlags::empty(),
        }
    }

    pub fn new_named(name: impl Into<TypeName>) -> Self {
        Self {
            hooks: const {
                tnr! {size_of::<C>() != 0 => Some(HooksBuilder::new()) : None}
            },
            name: Some(name.into()),
            flags: ComponentFlags::empty(),
        }
    }

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
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
        self.hooks = self.hooks.map(|b| b.with_default(f));
        self
    }

    pub fn clone(mut self, f: fn(&C) -> C) -> Self {
        const_assert!(|C| size_of::<C>() != 0, "can't set clone hook for ZST");
        self.hooks = self.hooks.map(|b| b.with_clone(f));
        self
    }

    pub fn on_add<F>(mut self, f: F) -> Self
    where
        F: FnMut(Entity) + 'static,
    {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_add hook for ZST");
        self.hooks = self.hooks.map(|b: HooksBuilder<C>| b.with_add(f));
        self
    }

    pub fn on_set<F>(mut self, f: F) -> Self
    where
        F: FnMut(Entity, &mut C) + 'static,
    {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_set hook for ZST");
        self.hooks = self.hooks.map(|b| b.with_set(f));
        self
    }

    pub fn on_remove<F>(mut self, f: F) -> Self
    where
        F: FnMut(Entity, &mut C) + 'static,
    {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_remove hook for ZST");
        self.hooks = self.hooks.map(|b| b.with_remove(f));
        self
    }

    pub fn build(self, world: &mut World) -> Entity {
        todo!()
    }
}
