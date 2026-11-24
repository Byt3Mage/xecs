use crate::{
    CycleError, Ecs, Fixed, HasMeta, Id, Many, One, Relation, Slot, bundle,
    data_structures::Sparse,
    id::allocator::IdRecord,
    relation::{
        self, Sources, Src,
        edge::{ComponentOf, Target},
        many::ManySlots,
        sealed::Sealed,
        slots::Slots,
    },
    table,
};

#[allow(private_interfaces)]
pub trait Targets: Sealed + 'static {
    type Pos: HasMeta;
    type Row<R: Relation>: HasMeta;
    type Store<R: Relation>: Default;

    fn get_mut<R>(ecs: &mut Ecs, r: IdRecord, pos: Self::Pos) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>;

    fn find<R>(ecs: &Ecs, r: IdRecord, tgt: Id) -> Option<&Target<R>>
    where
        R: Relation<Targets = Self>;

    fn find_mut<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>;

    fn for_each<R>(ecs: &Ecs, r: IdRecord, f: impl FnMut(&Target<R>))
    where
        R: Relation<Targets = Self>;

    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Targets = Self>;

    fn remove<R>(ecs: &mut Ecs, src: Id, pos: Self::Pos)
    where
        R: Relation<Targets = Self>;
}

#[allow(private_interfaces)]
impl Targets for One {
    type Pos = ();
    type Row<R: Relation> = Target<R>;
    type Store<R: Relation> = ();

    #[inline(always)]
    fn get_mut<R>(ecs: &mut Ecs, r: IdRecord, _: Self::Pos) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        relation::row_mut::<R>(ecs, r)
    }

    #[inline(always)]
    fn find<R>(ecs: &Ecs, r: IdRecord, tgt: Id) -> Option<&Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        relation::row::<R>(ecs, r).filter(|e| e.id == tgt)
    }

    #[inline(always)]
    fn find_mut<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        relation::row_mut::<R>(ecs, r).filter(|e| e.id == tgt)
    }

    #[inline(always)]
    fn for_each<R>(ecs: &Ecs, r: IdRecord, mut f: impl FnMut(&Target<R>))
    where
        R: Relation<Targets = Self>,
    {
        if let Some(tgt) = relation::row::<R>(ecs, r) {
            f(tgt)
        }
    }

    #[inline]
    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Targets = Self>,
    {
        relation::check_targets::<R>(ecs, tgt, src)
    }

    #[inline(always)]
    fn remove<R>(ecs: &mut Ecs, src: Id, _: Self::Pos)
    where
        R: Relation<Targets = Self>,
    {
        bundle::remove::<ComponentOf<R>>(ecs, ecs.ids[src]);
    }
}

#[allow(private_interfaces)]
impl Targets for Many {
    type Pos = u32;
    type Row<R: Relation> = ();
    type Store<R: Relation> = Sparse<ManySlots<Target<R>>>;

    fn get_mut<R>(ecs: &mut Ecs, r: IdRecord, pos: Self::Pos) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        let src = table::get_id(ecs, r);
        ecs.relations.get_mut::<R>().targets.get_mut(src).map(|s| &mut s[pos])
    }

    fn find<R>(ecs: &Ecs, r: IdRecord, tgt: Id) -> Option<&Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        let src = table::get_id(ecs, r);
        let targets = ecs.relations.get::<R>().targets.get(src)?;
        targets.iter().find(|t| t.id == tgt)
    }

    fn find_mut<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        let src = table::get_id(ecs, r);
        let targets = ecs.relations.get_mut::<R>().targets.get_mut(src)?;
        targets.iter_mut().find(|t| t.id == tgt)
    }

    fn for_each<R>(ecs: &Ecs, r: IdRecord, f: impl FnMut(&Target<R>))
    where
        R: Relation<Targets = Self>,
    {
        let src = table::get_id(ecs, r);
        if let Some(targets) = ecs.relations.get::<R>().targets.get(src) {
            targets.iter().for_each(f);
        }
    }

    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Targets = Self>,
    {
        Src::<R>::closes_cycle::<R>(ecs, src, tgt)
    }

    fn remove<R>(ecs: &mut Ecs, src: Id, pos: Self::Pos)
    where
        R: Relation<Targets = Self>,
    {
        let targets = &mut ecs.relations.get_mut::<R>().targets;

        if let Some(&tgt) = targets[src].swap_remove(pos) {
            let tgt = Src::<R>::get_mut::<R>(ecs, tgt.id, tgt.pos).expect("xecs: target must have a source list");
            tgt.pos = pos;
        }

        let targets = &mut ecs.relations.get_mut::<R>().targets;

        if targets[src].is_empty() {
            targets.remove(src);

            if let Ok(r) = ecs.ids.get(src) {
                bundle::remove::<ComponentOf<R>>(ecs, r);
            }
        }
    }
}

#[allow(private_interfaces)]
impl<const N: usize> Targets for Fixed<N> {
    type Pos = Slot<N>;
    type Row<R: Relation> = Slots<Target<R>, N>;
    type Store<R: Relation> = ();

    fn get_mut<R>(ecs: &mut Ecs, r: IdRecord, pos: Self::Pos) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        relation::row_mut::<R>(ecs, r)?.get_mut(pos)
    }

    fn find<R>(ecs: &Ecs, r: IdRecord, tgt: Id) -> Option<&Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        relation::row::<R>(ecs, r)?.iter().find(|t| t.id == tgt)
    }

    fn find_mut<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Option<&mut Target<R>>
    where
        R: Relation<Targets = Self>,
    {
        relation::row_mut::<R>(ecs, r)?.iter_mut().find(|t| t.id == tgt)
    }

    fn for_each<R>(ecs: &Ecs, r: IdRecord, f: impl FnMut(&Target<R>))
    where
        R: Relation<Targets = Self>,
    {
        if let Some(slots) = relation::row::<R>(ecs, r) {
            slots.iter().for_each(f);
        }
    }

    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Targets = Self>,
    {
        Src::<R>::closes_cycle::<R>(ecs, src, tgt)
    }

    fn remove<R>(ecs: &mut Ecs, src: Id, pos: Self::Pos)
    where
        R: Relation<Targets = Self>,
    {
        if let Ok(r) = ecs.ids.get(src)
            && let Some(slots) = relation::row_mut::<R>(ecs, r)
        {
            slots.take(pos);

            if slots.is_empty() {
                bundle::remove::<ComponentOf<R>>(ecs, r);
            }
        }
    }
}
