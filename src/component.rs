use std::{collections::HashMap, fmt::Debug, marker::PhantomData, rc::Rc};
use const_assert::const_assert;
use simple_ternary::tnr;
use crate::{
    component_flags::ComponentFlags, component_index::component_hash, entity::{add_flag, Entity, ECS_ANY, ECS_FLAG, ECS_WILDCARD}, entity_flags::EntityFlags, id::{is_pair, is_wildcard, pair, pair_first, pair_second, strip_generation, Id, COMPONENT_MASK, ID_FLAGS_MASK}, storage::{archetype::inc_traversable, archetype_index::ArchetypeId}, type_info::{TypeHooksBuilder, TypeInfo, TypeName}, world::{World, WorldRef}
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
}

impl ComponentBuilder {
    pub(crate) fn new(id: Option<Id>) -> Self {
        Self {
            id,
            name: None,
            flags: ComponentFlags::empty(),
        }
    }

    pub fn new_named(id: Option<Id>, name: impl Into<TypeName>) -> Self {
        Self {
            id,
            name: Some(name.into()),
            flags: ComponentFlags::empty(),
        }
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

    pub(crate) fn build(self, world: &mut World) -> Id {
        let id = self.id.unwrap_or_else(||world.new_entity());
        
        debug_assert!(!world.components.contains(id), "component already exists");

        let mut cr = ComponentRecord::new(id, self.flags, None);
        let mut rel;
        let mut tgt = 0;

        let is_wildcard = is_wildcard(id);
        let is_pair = is_pair(id);

        if is_pair {
            rel = pair_first(id) as u64;
            rel = world.entity_index.get_current( rel);

            assert!(rel != 0, "INTERNAL ERROR: null entity used as relationship");

            tgt = pair_second(id) as u64;

            assert!(rel != 0, "INTERNAL ERROR: null entity used as target");

            if !is_wildcard {
                /* Inherit flags from (relationship, *) record */
                cr.flags |= ensure_component(world, pair(rel, ECS_WILDCARD)).flags;

                /* Initialize type info if id is not a tag*/
                if !cr.flags.contains(ComponentFlags::TAG) {
                    let ty_idx = &world.type_index;
                    cr.type_info = ty_idx.get(rel).or_else(||tnr!{tgt != 0 => ty_idx.get(tgt) : None }).map(Rc::clone);
                }
            }
        }
        else {
            rel = id & COMPONENT_MASK;
            assert!(rel != 0, "INTERNAL ERROR: null entity can't be registered");
        }

        /* Flag for OnDelete policies */
        add_flag(world, rel, EntityFlags::IS_ID);

        if tgt != 0 {
            /* Flag for OnDeleteTarget policies */
            let tgt_r = world.entity_index.get_any_record_mut( tgt).unwrap();
            
            tgt_r.flags |= EntityFlags::IS_TARGET;

            if cr.flags.contains(ComponentFlags::TRAVERSABLE) {
                /* Flag used to determine if object should be traversed when
                * propagating events or with super/subset queries */
                if !tgt_r.flags.contains(EntityFlags::IS_TRAVERSABLE) {
                    let arch = world.archetypes.get_mut(tgt_r.arch).unwrap();
                    inc_traversable(arch, 1);
                }

                tgt_r.flags |= EntityFlags::IS_TRAVERSABLE;
                /* Add reference to (*, tgt) component record to entity record */
                tgt_r.cr = Some(component_hash(tgt));
            }

            /* If second element of pair determines the type, check if the pair 
            * should be stored as a sparse component.*/
            match &cr.type_info {
                Some(ti) => {
                    if ti.id == tgt {
                        let cr_t = ensure_component(world, tgt);

                        if cr_t.flags.contains(ComponentFlags::IS_SPARSE) {
                            cr.flags |= ComponentFlags::IS_SPARSE;
                        }
                    }
                }
                _ => {}
            }
        }
        // TODO: create type info.
        // TODO: add to components map.
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

    pub fn build(self, world: &mut World) -> Id {
        debug_assert!(!world.type_ids.has_t::<C>(), "component already exists.");

        let id = world.new_entity();
        let hash = component_hash(id);

        //let cr = ComponentRecord::new(id, self.flags);

        id
    }
}

pub(crate) fn ensure_component(world: &mut World, id: Id) -> &ComponentRecord {
    todo!()
}

pub(crate) fn get_component(world: &World, id: Id) -> Option<&ComponentRecord> {
    // TODO: revisit this.
    /*
    if (id == ecs_pair(EcsIsA, EcsWildcard)) {
        return world->cr_isa_wildcard;
    } else if (id == ecs_pair(EcsChildOf, EcsWildcard)) {
        return world->cr_childof_wildcard;
    } else if (id == ecs_pair_t(EcsIdentifier, EcsName)) {
        return world->cr_identifier_name;
    }
    */
    
    world.components.get(id)
}

pub(crate) fn get_component_mut(world: &mut World, id: Id) -> Option<&mut ComponentRecord> {
    // TODO: revisit this.
    /*
    if (id == ecs_pair(EcsIsA, EcsWildcard)) {
        return world->cr_isa_wildcard;
    } else if (id == ecs_pair(EcsChildOf, EcsWildcard)) {
        return world->cr_childof_wildcard;
    } else if (id == ecs_pair_t(EcsIdentifier, EcsName)) {
        return world->cr_identifier_name;
    }
    */
    
    let hash = component_hash(id);
    todo!();
}