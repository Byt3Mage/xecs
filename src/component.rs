use std::{alloc::Layout, any::{type_name, TypeId}, collections::HashMap, marker::PhantomData};
use const_assert::const_assert;
use crate::{component_flags::ComponentFlags, entity::Entity, error::{EcsError, EcsResult}, id::Id, storage::archetype_index::ArchetypeId, type_info::{TypeHooks, TypeHooksBuilder, TypeInfo, TypeName}, world::{World, WorldRef}};

pub trait ComponentValue: 'static {}

pub struct TypedComponentView<'a, C: ComponentValue> {
    id: Id,
    world: WorldRef<'a>,
    _phantom: PhantomData<fn()-> C>
}

impl <'a, T: ComponentValue> TypedComponentView<'a, T> {
    pub(crate) fn new(world: impl Into<WorldRef<'a>>, id: Id) -> Self {
        Self {
            id,
            world: world.into(),
            _phantom: Default::default()
        }
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
    pub fn new(id: Id) -> Self {
        Self {
            id,
            flags: ComponentFlags::empty(),
            archetypes: HashMap::new(),
        }
    }
}

pub struct ComponentBuilder {
    name: Option<TypeName>,
    tags: Vec<Id>,
    typed_tags: Vec<TypeId>,
    type_layout: Option<Layout>,
    type_hooks: Option<TypeHooks>,
}

impl ComponentBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            tags: vec![],
            typed_tags: vec![],
            type_layout: None,
            type_hooks: None, 
        }
    }

    pub fn new_named(name: impl Into<TypeName>) -> Self {
        Self {
            name: Some(name.into()),
            tags: vec![],
            typed_tags: vec![],
            type_layout: None,
            type_hooks: None,
        }
    }

    pub fn with_type<C: ComponentValue>(mut self, hooks: TypeHooksBuilder<C>) -> Self {
        self.type_layout = Some(Layout::new::<C>());
        self.type_hooks = Some(hooks.build());
        self
    }

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn add(mut self, id: Id) -> Self {
        self.tags.push(id);
        self
    }

    pub fn add_t<T: ComponentValue>(mut self) -> Self {
        self.typed_tags.push(TypeId::of::<T>());
        self
    }

    pub(crate) fn build(self, world: &mut World) -> EcsResult<ComponentView> {
        let id = world.entity_index.new_id();
        
        let tags = self.typed_tags.into_iter()
        .map(|ty_id| world.type_ids.get_id(ty_id).unwrap())
        .chain(self.tags.into_iter())
        .collect::<Vec<_>>();

        match (self.type_layout, self.type_hooks) {
            (Some(layout), Some(hooks)) => {
                let type_info = TypeInfo {
                    id,
                    layout,
                    hooks,
                    type_name: self.name
                };
            }
            _=> {}
        }

        for tag in tags {
            world.add_id(id, tag)?;
        }

        Ok(ComponentView::new(world, id))
    }
}

pub struct TypedComponentBuilder<C> {
    hook_builder: TypeHooksBuilder<C>,
    tags: Vec<Id>,
    typed_tags: Vec<TypeId>,
    name: Option<TypeName>
}

impl <C: ComponentValue> TypedComponentBuilder<C> {
    pub fn new() -> Self {
        const_assert!(|C| std::mem::size_of::<C>() != 0, "use ComponentBuilder for ZST.");

        Self{
            hook_builder: TypeHooksBuilder::new(),
            tags: vec![],
            typed_tags: vec![],
            name: None,
        }
    }

    pub fn new_named(name: impl Into<TypeName>) -> Self {
        const_assert!(|C| std::mem::size_of::<C>() != 0, "use ComponentBuilder for ZST.");

        Self{
            hook_builder: TypeHooksBuilder::new(),
            tags: vec![],
            typed_tags: vec![],
            name: Some(name.into()),
        }
    }

    pub fn name(mut self, name: impl Into<TypeName>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn add(mut self, id: Id) -> Self {
        self.tags.push(id);
        self
    }

    pub fn add_t<T: ComponentValue>(mut self) -> Self {
        self.typed_tags.push(TypeId::of::<T>());
        self
    }

    pub fn default(mut self, f: fn() -> C) -> Self {
        self.hook_builder = self.hook_builder.with_default(f);
        self
    }

    pub fn clone(mut self, f: fn(&C) -> C) -> Self {
        self.hook_builder = self.hook_builder.with_clone(f);
        self
    }

    fn on_add<F>(mut self, f: F) -> Self 
    where F: FnMut(Entity) + 'static {
        self.hook_builder = self.hook_builder.with_add(f);
        self
    }

    pub fn on_set<F>(mut self, f: F) -> Self 
    where F: FnMut(Entity, &mut C) + 'static {
        self.hook_builder = self.hook_builder.with_set(f);
        self
    }

    pub fn on_remove<F>(mut self, f: F) -> Self 
    where F: FnMut(Entity, &mut C) + 'static {
        self.hook_builder = self.hook_builder.with_remove(f);
        self
    }

    pub(crate) fn build(self, world: &mut World) -> EcsResult<TypedComponentView<C>> {
        if world.type_ids.get_id_t::<C>().is_some() {
            return Err(EcsError::ComponentCreate("component already exists"))
        }

        let id = world.entity_index.new_id();
        
        let tags = self.typed_tags.into_iter()
        .map(|ty_id| world.type_ids.get_id(ty_id).unwrap())
        .chain(self.tags.into_iter())
        .collect::<Vec<_>>();

        let type_info = TypeInfo {
            id,
            layout: Layout::new::<C>(),
            type_name: Some(self.name.unwrap_or(type_name::<C>().into())),
            hooks: self.hook_builder.build(),
        };

        for tag in tags {
            world.add_id(id, tag)?;
        }

        Ok(TypedComponentView::new(world, id))
    }
}