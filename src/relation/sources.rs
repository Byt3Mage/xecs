use crate::{
    Ecs, HasMeta, Id, Many, One, RelateError, Relation,
    relation::{
        Tgt, Unbounded, directed,
        edge::{Source, TPos},
        many::ManyStore,
        sealed::Sealed,
        targets::Targets,
    },
};

/// Edges per target, and the reverse index that follows from it.
pub trait Sources: Sealed + Unbounded + 'static {
    type Pos: HasMeta;
    type Store<R: Relation>;

    fn get_mut<R>(ecs: &mut Ecs, tgt: Id, pos: Self::Pos) -> Option<&mut Source<R>>
    where
        R: Relation<Sources = Self>;

    fn insert<R>(ecs: &mut Ecs, tgt: Id, src: Source<R>) -> Self::Pos
    where
        R: Relation<Sources = Self>;

    fn remove<R>(ecs: &mut Ecs, tgt: Id, pos: Self::Pos) -> TPos<R>
    where
        R: Relation<Sources = Self>;

    fn relate<R>(ecs: &mut Ecs, src: Id, tgt: Id, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded;

    fn unrelate<R>(ecs: &mut Ecs, src: Id, tgt: Id)
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded;
}

impl Sources for One {
    type Pos = ();
    type Store<R: Relation> = Source<R>;

    #[inline(always)]
    fn get_mut<R>(ecs: &mut Ecs, tgt: Id, _: Self::Pos) -> Option<&mut Source<R>>
    where
        R: Relation<Sources = Self>,
    {
        ecs.relations.get_mut::<R>().sources.get_mut(tgt)
    }

    #[inline(always)]
    fn insert<R>(ecs: &mut Ecs, tgt: Id, src: Source<R>) -> Self::Pos
    where
        R: Relation<Sources = Self>,
    {
        ecs.relations.get_mut::<R>().sources.set(tgt, src);
    }

    #[inline(always)]
    fn remove<R>(ecs: &mut Ecs, tgt: Id, _: Self::Pos) -> TPos<R>
    where
        R: Relation<Sources = Self>,
    {
        let sources = &mut ecs.relations.get_mut::<R>().sources;
        let pos = sources[tgt].pos;
        sources.remove(tgt);
        pos
    }

    fn relate<R>(ecs: &mut Ecs, src: Id, tgt: Id, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded,
    {
        directed::relate::<R>(ecs, src, tgt, data)
    }

    fn unrelate<R>(ecs: &mut Ecs, src: Id, tgt: Id)
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded,
    {
        directed::unrelate::<R>(ecs, src, tgt)
    }
}

impl Sources for Many {
    type Pos = u32;
    type Store<R: Relation> = ManyStore<Source<R>>;

    #[inline(always)]
    fn get_mut<R>(ecs: &mut Ecs, tgt: Id, pos: Self::Pos) -> Option<&mut Source<R>>
    where
        R: Relation<Sources = Self>,
    {
        ecs.relations.get_mut::<R>().sources.get_mut(tgt).map(|t| &mut t[pos])
    }

    fn insert<R>(ecs: &mut Ecs, tgt: Id, src: Source<R>) -> Self::Pos
    where
        R: Relation<Sources = Self>,
    {
        ecs.relations
            .get_mut::<R>()
            .sources
            .get_or_insert_with(tgt, ManyStore::new)
            .push(src)
    }

    fn remove<R>(ecs: &mut Ecs, tgt: Id, pos: Self::Pos) -> TPos<R>
    where
        R: Relation<Sources = Self>,
    {
        let sources = &mut ecs.relations.get_mut::<R>().sources[tgt];
        let src = sources[pos];

        if let Some(&mov) = sources.swap_remove(pos) {
            let r = ecs.ids.get(mov.id).expect("xecs: source is alive");
            let tgt = Tgt::<R>::get_mut::<R>(ecs, r, mov.pos).expect("xecs: source has target");
            tgt.pos = pos;
        }

        let sources = &mut ecs.relations.get_mut::<R>().sources;

        if sources[tgt].is_empty() {
            sources.remove(tgt);
        }

        src.pos
    }

    fn relate<R>(ecs: &mut Ecs, src: Id, tgt: Id, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded,
    {
        directed::relate::<R>(ecs, src, tgt, data)
    }

    fn unrelate<R>(ecs: &mut Ecs, src: Id, tgt: Id)
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded,
    {
        directed::unrelate::<R>(ecs, src, tgt);
    }
}
