use crate::{
    Bounded, CycleError, Ecs, HasMeta, Id, Many, One, RelateError, Relation,
    relation::{
        self, Tgt, Unbounded, directed,
        edge::{Source, TPos},
        many::ManySlots,
        sealed::Sealed,
        targets::Targets,
    },
};

/// Edges per target, and the reverse index that follows from it.
pub trait Sources: Sealed + 'static {
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

    fn relate_at<R>(ecs: &mut Ecs, src: Id, tgt: Id, pos: TPos<R>, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Bounded;

    fn unrelate<R>(ecs: &mut Ecs, src: Id, tgt: Id)
    where
        R: Relation<Sources = Self>;

    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Sources = Self>;
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
        ecs.relations.get_mut::<R>().sources.remove(tgt).unwrap().pos
    }

    fn relate<R>(ecs: &mut Ecs, src: Id, tgt: Id, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Unbounded,
    {
        directed::relate::<R>(ecs, src, tgt, data)
    }

    fn relate_at<R>(ecs: &mut Ecs, src: Id, tgt: Id, pos: TPos<R>, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Bounded,
    {
        directed::relate_at::<R>(ecs, src, tgt, pos, data)
    }

    fn unrelate<R>(ecs: &mut Ecs, src: Id, tgt: Id)
    where
        R: Relation<Sources = Self>,
    {
        directed::unrelate::<R>(ecs, src, tgt)
    }

    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Sources = Self>,
    {
        relation::check_sources::<R>(ecs, src, tgt)
    }
}

impl Sources for Many {
    type Pos = u32;
    type Store<R: Relation> = ManySlots<Source<R>>;

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
            .get_or_insert_with(tgt, ManySlots::new)
            .push(src)
    }

    fn remove<R>(ecs: &mut Ecs, tgt: Id, pos: Self::Pos) -> TPos<R>
    where
        R: Relation<Sources = Self>,
    {
        let sources = &mut ecs.relations.get_mut::<R>().sources[tgt];
        let src = sources[pos];

        if let Some(&mov) = sources.swap_remove(pos) {
            let r = ecs.ids[mov.id];
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

    fn relate_at<R>(ecs: &mut Ecs, src: Id, tgt: Id, pos: TPos<R>, data: R::Data) -> Result<(), RelateError<R>>
    where
        R: Relation<Sources = Self>,
        R::Targets: Bounded,
    {
        directed::relate_at::<R>(ecs, src, tgt, pos, data)
    }

    fn unrelate<R>(ecs: &mut Ecs, src: Id, tgt: Id)
    where
        R: Relation<Sources = Self>,
    {
        directed::unrelate::<R>(ecs, src, tgt);
    }

    #[inline]
    fn closes_cycle<R>(ecs: &mut Ecs, src: Id, tgt: Id) -> Result<(), CycleError>
    where
        R: Relation<Sources = Self>,
    {
        relation::check_targets_dfs::<R>(ecs, tgt, src)
    }
}
