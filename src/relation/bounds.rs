use crate::{
    Ecs, Fixed, Id, Many, One, Relation, Targets, bundle,
    id::allocator::IdRecord,
    relation::{
        self, Sources, Src,
        edge::{EdgeField, TPos, Target},
        many::ManySlots,
        sealed::Sealed,
        slots::Slots,
    },
    table,
};

pub enum Entry<'a, R: Relation> {
    Filled(&'a mut Target<R>),
    Insert(TPos<R>),
}

#[allow(private_interfaces)]
pub trait Unbounded: Sealed + Targets + 'static {
    fn entry<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Entry<'_, R>
    where
        R: Relation<Targets = Self>;

    fn insert<R>(ecs: &mut Ecs, src: Id, tgt: Target<R>)
    where
        R: Relation<Targets = Self>;
}

#[allow(private_interfaces)]
impl Unbounded for One {
    fn entry<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Entry<'_, R>
    where
        R: Relation<Targets = Self>,
    {
        match relation::row_mut::<R>(ecs, r) {
            Some(t) if t.id == tgt => Entry::Filled(t),
            _ => Entry::Insert(()),
        }
    }

    fn insert<R>(ecs: &mut Ecs, src: Id, tgt: Target<R>)
    where
        R: Relation<Targets = Self>,
    {
        let r = ecs.ids.get(src).expect("xecs: relation source is alive");

        match relation::row_mut::<R>(ecs, r) {
            None => bundle::insert(ecs, r, EdgeField::<R>(tgt)),
            Some(t) => {
                let prev = std::mem::replace(t, tgt);
                Src::<R>::remove::<R>(ecs, prev.id, prev.pos);
            }
        };
    }
}

#[allow(private_interfaces)]
impl Unbounded for Many {
    fn entry<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Entry<'_, R>
    where
        R: Relation<Targets = Self>,
    {
        let src = table::get_id(ecs, r);
        let store = ecs.relations.get_mut::<R>();

        let Some(targets) = store.targets.get_mut(src) else {
            return Entry::Insert(0);
        };

        let next = targets.next_slot();

        match targets.iter_mut().find(|t| t.id == tgt) {
            Some(t) => Entry::Filled(t),
            None => Entry::Insert(next),
        }
    }

    fn insert<R>(ecs: &mut Ecs, src: Id, tgt: Target<R>)
    where
        R: Relation<Targets = Self>,
    {
        let r = ecs.ids.get(src).expect("xecs: relation source is alive");
        bundle::insert(ecs, r, EdgeField::<R>(()));
        ecs.relations
            .get_mut::<R>()
            .targets
            .get_or_insert_with(src, ManySlots::new)
            .push(tgt);
    }
}

pub trait Bounded: Sealed + Targets + 'static {
    fn insert<R>(ecs: &mut Ecs, src: Id, pos: Self::Pos, tgt: Target<R>)
    where
        R: Relation<Targets = Self>;
}

impl<const N: usize> Bounded for Fixed<N> {
    fn insert<R>(ecs: &mut Ecs, src: Id, pos: Self::Pos, tgt: Target<R>)
    where
        R: Relation<Targets = Self>,
    {
        let r = ecs.ids.get(src).expect("xecs: relation source is alive");

        match relation::row_mut::<R>(ecs, r) {
            None => bundle::insert(ecs, r, EdgeField::<R>(Slots::with_entry(pos, tgt))),
            Some(slots) => {
                if let Some(prev) = slots.replace(pos, tgt) {
                    Src::<R>::remove::<R>(ecs, prev.id, prev.pos);
                }
            }
        };
    }
}
