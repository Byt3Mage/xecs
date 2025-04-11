use crate::{component::ComponentRecord, entity::{Entity, ECS_ANY, ECS_WILDCARD}, id::{is_pair, pair, pair_first, pair_second, strip_generation, Id, COMPONENT_MASK}, world::World};

const fn component_hash(id: Id) -> Id {
    let mut id = strip_generation(id);

    if is_pair(id) {
        let mut rel = pair_first((id) & COMPONENT_MASK) as u64;
        let mut obj = pair_second(id) as u64;

        if rel == ECS_ANY {
            rel = ECS_WILDCARD;
        }

        if obj == ECS_ANY {
            obj = ECS_WILDCARD;
        }

        id = pair(rel, obj);
    }

    id
}

pub fn ensure_component(world: &mut World, id: Id) -> &ComponentRecord {
    let hash = component_hash(id);

    if !world.component_index.contains_key(&hash) {
        let new_comp = new_component(world, id);
        world.component_index.insert(hash, new_comp);
    }
    
    world.component_index.get_mut(&hash).unwrap()
}

fn new_component(world: &mut World, id: Id) -> ComponentRecord {
    todo!()
}

pub fn get_component(world: &World, id: Id) -> &ComponentRecord {
    todo!()
}