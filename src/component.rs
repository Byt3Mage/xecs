use std::{alloc::Layout, any::TypeId, collections::HashMap, fmt::Debug, marker::PhantomData, rc::Rc};
use const_assert::const_assert;
use simple_ternary::tnr;
use crate::{
    entity::{Entity, ECS_ANY, ECS_WILDCARD}, 
    flags::ComponentFlags, 
    id::Id, 
    storage::archetype_index::ArchetypeId, 
    type_info::{TypeHooksBuilder, TypeInfo, TypeName}, 
    world::{World, WorldRef}
};

pub trait ComponentValue: 'static {}

impl <T: 'static> ComponentValue for T {}

pub struct TypedComponentView<'a, C: ComponentValue> {
    id: Id,
    world: WorldRef<'a>,
    phantom: PhantomData<fn()-> C>
}

impl <'a, T: ComponentValue> TypedComponentView<'a, T> {
    pub(crate) fn new(world: impl Into<WorldRef<'a>>, id: Id) -> Self {
        Self {
            id,
            world: world.into(),
            phantom: PhantomData
        }
    }

    #[inline]
    pub fn id(&self) -> Id {
        self.id
    }
}

pub struct ComponentView<'a> {
    id: Id,
    world: WorldRef<'a>
}

impl <'a> ComponentView <'a> {
    pub(crate) fn new(world: impl Into<WorldRef<'a>>, id: Id) -> Self {
        Self {
            id,
            world: world.into(),
        }
    }

    #[inline]
    pub fn id(&self) -> Id {
        self.id
    }
}

/// Component location info within an [Archetype](crate::storage::archetype::Archetype).
pub(crate) struct ComponentLocation {
    /// First index of id within the archetype's [Type](crate::type_info::Type).
    pub id_index: usize,
    /// Number of times the id occurs in the archetype. E.g id, (id, \*), (\*, id).
    pub id_count: usize,
    /// First [Column](crate::storage::archetype_data::Column) index where the id appears (if not tag).
    pub column_index: Option<usize>,
}

pub struct ComponentRecord {
    pub id: Id,
    pub flags: ComponentFlags,
    pub type_info: Option<Rc<TypeInfo>>,
    pub archetypes: HashMap<ArchetypeId, ComponentLocation>
    
}

impl ComponentRecord {
    pub(crate) fn new(id: Id, flags: ComponentFlags, ti: Option<Rc<TypeInfo>>) -> Self {
        Self {
            id,
            flags,
            type_info: ti,
            archetypes: HashMap::new(),
        }
    }
}

pub struct ComponentBuilder {
    id: Option<Id>,
    name: Option<TypeName>,
    flags: ComponentFlags,
    type_info: Option<TypeInfo>,
}

impl ComponentBuilder {
    pub(crate) fn new(id: Option<Id>) -> Self {
        Self {
            id,
            name: None,
            flags: ComponentFlags::empty(),
            type_info: None,
        }
    }

    pub fn new_named(id: Option<Id>, name: impl Into<TypeName>) -> Self {
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

    pub fn set_type<C: ComponentValue>(mut self, hooks: TypeHooksBuilder<C>) -> Self {
        self.type_info = Some(TypeInfo { 
            id: 0, 
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

    pub(crate) fn build(self, world: &mut World) -> Id {
        let id = self.id.unwrap_or_else(||world.new_entity());

        debug_assert!(!world.components.contains(id), "component already exists");
        assert!(id != 0 && id != ECS_WILDCARD && id != ECS_ANY, "INVALID ID: component id is null or forbidden");

        let mut cr = ComponentRecord::new(id, self.flags, None);
        
        if let Some(mut ti) = self.type_info {
            ti.id = id;
            ti.type_name = self.name;// TODO: add scoped names.
            let ti = Rc::new(ti);

            cr.type_info = Some(Rc::clone(&ti));
            world.type_index.insert(id, ti);
        }

        world.components.insert(id, cr);
        id
    }
}

impl Debug for ComponentBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComponentBuilder").field("id", &self.id).finish()
    }
}

pub struct TypedComponentBuilder<C> {
    hooks: Option<TypeHooksBuilder<C>>,
    name: Option<TypeName>,
    flags: ComponentFlags,
}

impl <C: ComponentValue> TypedComponentBuilder<C> {
    pub(crate) fn new() -> Self {
        Self{
            hooks: const { tnr!{size_of::<C>() != 0 => Some(TypeHooksBuilder::new()) : None} },
            name: None,
            flags: ComponentFlags::empty(),
        }
    }

    pub fn new_named(name: impl Into<TypeName>) -> Self {
        Self{
            hooks: const { tnr!{size_of::<C>() != 0 => Some(TypeHooksBuilder::new()) : None} },
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
    where F: FnMut(Entity) + 'static {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_add hook for ZST");
        self.hooks = self.hooks.map(|b: TypeHooksBuilder<C>| b.with_add(f));
        self
    }

    pub fn on_set<F>(mut self, f: F) -> Self 
    where F: FnMut(Entity, &mut C) + 'static {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_set hook for ZST");
        self.hooks = self.hooks.map(|b| b.with_set(f));
        self
    }

    pub fn on_remove<F>(mut self, f: F) -> Self 
    where F: FnMut(Entity, &mut C) + 'static {
        const_assert!(|C| size_of::<C>() != 0, "can't set on_remove hook for ZST");
        self.hooks = self.hooks.map(|b| b.with_remove(f));
        self
    }

    pub fn build(self, world: &mut World) -> Id {
        debug_assert!(!world.type_ids.has_t::<C>(), "component already exists.");

        let id = world.new_entity();
        let mut cr = ComponentRecord::new(id, self.flags, None);
        
        if let Some(hooks) = self.hooks {
            let ti = Rc::new(TypeInfo {
                id,
                layout: Layout::new::<C>(),
                hooks: hooks.build(),
                type_name: Some(self.name.unwrap_or(std::any::type_name::<C>().into())), // TODO: scoped name.
                type_id: TypeId::of::<C>(),
            });

            cr.type_info = Some(Rc::clone(&ti));
            world.type_index.insert(id, ti);
        };

        world.type_ids.set_t::<C>(id);
        world.components.insert(id, cr);

        id
    }
}