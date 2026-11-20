use xecs::{Acyclic, Ecs, Many, One, schema};

schema! {
     relation ChildOf {
        targets: One,
        sources: Many,
        cycles: Acyclic,
    }
}

#[test]
fn relation_add_get_remove() {
    let mut ecs = Ecs::new();

    let bob = ecs.empty().unwrap();
    let alice = ecs.empty().unwrap();

    ecs.relate::<ChildOf>(bob, alice, ()).unwrap();

    let tgt = ecs.target::<ChildOf>(bob).unwrap();

    assert_eq!(tgt, Some(alice));

    ecs.targets::<ChildOf>(bob, |id, _| println!("{id} is a parent of bob"))
        .unwrap();

    ecs.unrelate::<ChildOf>(bob, alice);

    let tgt = ecs.target::<ChildOf>(bob).unwrap();

    assert_eq!(tgt, None);
}
