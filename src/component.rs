use std::{alloc::Layout, any::{type_name, TypeId}, collections::HashMap, fmt::Debug, marker::PhantomData, rc::Rc};
use const_assert::const_assert;
use crate::{component_flags::ComponentFlags, entity::Entity, id::Id, storage::archetype_index::ArchetypeId, type_info::{TypeHooks, TypeHooksBuilder, TypeInfo, TypeName}, world::{World, WorldRef}};

pub trait ComponentValue: 'static {}

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
    pub archetypes: HashMap<ArchetypeId, ComponentLocation>
}

impl ComponentRecord {
    pub(crate) fn new(id: Id, flags: ComponentFlags) -> Self {
        Self {
            id,
            flags,
            archetypes: HashMap::new(),
        }
    }
}

pub struct ComponentBuilder {
    id: Option<Id>,
    name: Option<TypeName>,
    type_info: Option<TypeInfo>,
    flags: ComponentFlags,
}

impl ComponentBuilder {
    pub(crate) fn new(id: Option<Id>) -> Self {
        Self {
            id,
            name: None,
            type_info: None,
            flags: ComponentFlags::empty(),
        }
    }

    pub fn new_named(id: Option<Id>, name: impl Into<TypeName>) -> Self {
        Self {
            id,
            name: Some(name.into()),
            type_info: None,
            flags: ComponentFlags::empty(),
        }
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

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
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

    pub fn build(self, world: &mut World) -> ComponentView {
        let id = match self.id {
            Some(id) => {
                debug_assert!(world.components.contains_key(&id), "component already exists.");
                id
            },
            None => world.new_entity().id(),
        };
        
        world.components.insert(id, ComponentRecord::new(id, self.flags));

        if let Some(mut type_info) = self.type_info {
            type_info.id = id;
            type_info.type_name = self.name; // TODO: add scoped names.

            world.type_infos.insert(id, Rc::new(type_info));
        }

        ComponentView::new(world, id)
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
        let hook_builder = const {
            if size_of::<C>() != 0 { Some(TypeHooksBuilder::new()) } else { None }
        };

        Self{
            hooks: hook_builder,
            name: None,
            flags: ComponentFlags::empty(),
        }
    }

    pub fn new_named(name: impl Into<TypeName>) -> Self {
        let hook_builder = const {
            if size_of::<C>() != 0 { Some(TypeHooksBuilder::new()) } else { None }
        };

        Self{
            hooks: hook_builder,
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

    pub fn build(self, world: &mut World) -> TypedComponentView<C> {
        debug_assert!(!world.type_ids.has_t::<C>(), "component already exists.");

        let id = world.new_entity().id();

        world.type_ids.set_t::<C>(id);
        world.components.insert(id, ComponentRecord::new(id, self.flags));

        if let Some(hooks) = self.hooks {
            world.type_infos.insert(id, Rc::new(TypeInfo {
                id,
                layout: Layout::new::<C>(),
                type_name: Some(self.name.unwrap_or(type_name::<C>().into())), // TODO: add scoped names
                hooks: hooks.build(),
                type_id: TypeId::of::<C>(),
            }));
        }
        
        TypedComponentView::new(world, id)
    }
}

impl <C> Debug for TypedComponentBuilder<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedComponentBuilder").field("type", &std::any::type_name::<C>()).finish()
    }
}