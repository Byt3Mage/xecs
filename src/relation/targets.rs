use crate::{
    Ecs, HasMeta, Id, Many, One, Relation, bundle,
    data_structures::Sparse,
    id::allocator::IdRecord,
    relation::{
        self, Sources, Src,
        edge::{ComponentOf, EdgeField, TPos, Target},
        many::ManyStore,
        sealed::Sealed,
    },
    table,
};

pub enum Entry<'a, R: Relation> {
    Filled(&'a mut Target<R>),
    Insert(TPos<R>),
}

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

    fn insert<R>(ecs: &mut Ecs, src: Id, tgt: Target<R>)
    where
        R: Relation<Targets = Self>;

    fn remove<R>(ecs: &mut Ecs, src: Id, pos: Self::Pos)
    where
        R: Relation<Targets = Self>;

    fn for_each<R>(ecs: &Ecs, r: IdRecord, f: impl FnMut(&Target<R>))
    where
        R: Relation<Targets = Self>;

    fn entry<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Entry<'_, R>
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

    fn insert<R>(ecs: &mut Ecs, id: Id, value: Target<R>)
    where
        R: Relation<Targets = Self>,
    {
        let r = ecs.ids.get(id).expect("xecs: relation source is alive");

        match relation::row_mut::<R>(ecs, r) {
            None => bundle::insert(ecs, r, EdgeField::<R>(value)),
            Some(tgt) => {
                let prev = std::mem::replace(tgt, value);
                Src::<R>::remove::<R>(ecs, prev.id, prev.pos);
            }
        };
    }

    fn remove<R>(ecs: &mut Ecs, src: Id, _: Self::Pos)
    where
        R: Relation<Targets = Self>,
    {
        let r = ecs.ids.get(src).expect("xecs: relation source is alive");
        bundle::remove::<ComponentOf<R>>(ecs, r);
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

    fn entry<R>(ecs: &mut Ecs, r: IdRecord, tgt: Id) -> Entry<'_, R>
    where
        R: Relation<Targets = Self>,
    {
        match relation::row_mut::<R>(ecs, r) {
            Some(t) if t.id == tgt => Entry::Filled(t),
            _ => Entry::Insert(()),
        }
    }
}

#[allow(private_interfaces)]
impl Targets for Many {
    type Pos = u32;
    type Row<R: Relation> = ();
    type Store<R: Relation> = Sparse<ManyStore<Target<R>>>;

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

    fn insert<R>(ecs: &mut Ecs, src: Id, tgt: Target<R>)
    where
        R: Relation<Targets = Self>,
    {
        ecs.relations
            .get_mut::<R>()
            .targets
            .get_or_insert_with(src, ManyStore::new)
            .push(tgt);
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
        }
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
}
