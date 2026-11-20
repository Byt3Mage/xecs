use crate::{
    Many, One, RelateError,
    ecs::Ecs,
    id::Id,
    relation::{
        Cycle, Cycles, Relation, Src, Tgt, Unbounded,
        bounds::Bounded,
        edge::{Source, TPos, Target},
        sources::Sources,
        targets::{Entry, Targets},
    },
};

/// Sources of a directed relation.
pub trait Directed: Sources {}

impl Directed for One {}
impl Directed for Many {}

#[inline(always)]
pub(crate) fn validate<R: Relation>(ecs: &mut Ecs, s: Id, t: Id) -> Result<(), RelateError<R>> {
    ecs.ids.validate_alive(t).map_err(RelateError::NotAlive)?;
    Cycle::<R>::check::<R>(ecs, s, t).map_err(RelateError::Cycles)?;
    Ok(())
}

pub(crate) fn relate<R: Relation>(ecs: &mut Ecs, s: Id, t: Id, data: R::Data) -> Result<(), RelateError<R>>
where
    R::Targets: Unbounded,
    R::Sources: Directed,
{
    let r = ecs.ids.get(s)?;

    let tpos = match Tgt::<R>::entry::<R>(ecs, r, t) {
        Entry::Filled(e) => {
            e.data = data;
            return Ok(());
        }
        Entry::Insert(e) => e,
    };

    validate::<R>(ecs, s, t)?;

    let spos = Src::<R>::insert::<R>(ecs, t, Source::<R>::source(s, tpos));
    Tgt::<R>::insert::<R>(ecs, s, Target::<R>::target(t, spos, data));

    Ok(())
}

pub(crate) fn unrelate<R: Relation>(ecs: &mut Ecs, s: Id, t: Id)
where
    R::Targets: Unbounded,
    R::Sources: Directed,
{
    let Ok(r) = ecs.ids.get(s) else { return };
    let Some(tgt) = Tgt::<R>::find::<R>(ecs, r, t) else { return };
    let tpos = Src::<R>::remove::<R>(ecs, t, tgt.pos);
    Tgt::<R>::remove::<R>(ecs, s, tpos);
}

pub(crate) fn relate_at<R: Relation>(
    ecs: &mut Ecs,
    s: Id,
    t: Id,
    pos: TPos<R>,
    data: R::Data,
) -> Result<(), RelateError<R>>
where
    R::Targets: Bounded,
    R::Sources: Directed,
{
    let r = ecs.ids.get(s)?;

    let tpos = match Tgt::<R>::entry::<R>(ecs, r, t) {
        Entry::Filled(e) => {
            e.data = data;
            return Ok(());
        }
        Entry::Insert(e) => e,
    };

    validate::<R>(ecs, s, t)?;

    let spos = Src::<R>::insert::<R>(ecs, t, Source::<R>::source(s, tpos));
    Tgt::<R>::insert::<R>(ecs, s, Target::<R>::target(t, spos, data));

    Ok(())
}
