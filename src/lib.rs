pub mod world;

mod component;
mod component_index;
mod entity_view;
mod entity_index;
mod error;
mod graph;
mod id;
mod storage;
mod type_info;
mod component_flags;
mod event_flags;
mod entity;
mod data_structures;
mod pointer;
mod utils;
mod world_utils;

mod tests {
    use crate::{component::ComponentValue, component_flags::ComponentFlags, entity, type_info::TypeHooksBuilder, world::World};

    struct MyStruct {x: f64}
    impl ComponentValue for MyStruct{}

    #[test]
    fn test() {
        let mut world = World::new();

        let comp = world.new_component().name("MyComp")
        .with_flag(ComponentFlags::CAN_TOGGLE | ComponentFlags::EXCLUSIVE)
        .set_type(TypeHooksBuilder::<MyStruct>::new())
        .build(&mut world).id();

        let id = world.component(comp).unwrap().id();

        assert_eq!(comp, id);
    }
}