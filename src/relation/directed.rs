use crate::{
    Many, One, RelateError,
    ecs::Ecs,
    id::Id,
    relation::{
        Cycle, Cycles, Relation, Src, Tgt,
        bounds::{Bounded, Entry, Unbounded},
        edge::{Source, TPos, Target},
        sources::Sources,
        targets::Targets,
    },
};

/// Sources of a directed relation.
pub trait Directed: Sources {}
impl Directed for One {}
impl Directed for Many {}

pub(crate) fn relate<R>(ecs: &mut Ecs, s: Id, t: Id, data: R::Data) -> Result<(), RelateError<R>>
where
    R: Relation<Targets: Unbounded, Sources: Directed>,
{
    let r = ecs.ids.get(s)?;

    let tpos = match Tgt::<R>::entry::<R>(ecs, r, t) {
        Entry::Filled(e) => {
            e.data = data;
            return Ok(());
        }
        Entry::Insert(e) => {
            ecs.ids.check_alive(t)?;
            Cycle::<R>::check::<R>(ecs, s, t).map_err(RelateError::Cycles)?;
            e
        }
    };

    let spos = Src::<R>::insert::<R>(ecs, t, Source::<R>::source(s, tpos));
    Tgt::<R>::insert::<R>(ecs, s, Target::<R>::target(t, spos, data));

    Ok(())
}

pub(crate) fn relate_at<R>(ecs: &mut Ecs, s: Id, t: Id, pos: TPos<R>, data: R::Data) -> Result<(), RelateError<R>>
where
    R: Relation<Targets: Bounded, Sources: Directed>,
{
    let r = ecs.ids.get(s)?;

    if let Some(tgt) = Tgt::<R>::get_mut::<R>(ecs, r, pos)
        && tgt.id == t
    {
        tgt.data = data;
        return Ok(());
    }

    ecs.ids.check_alive(t)?;
    Cycle::<R>::check::<R>(ecs, s, t).map_err(RelateError::Cycles)?;

    let spos = Src::<R>::insert::<R>(ecs, t, Source::<R>::source(s, pos));
    Tgt::<R>::insert::<R>(ecs, s, pos, Target::<R>::target(t, spos, data));
    Ok(())
}

pub(crate) fn unrelate<R>(ecs: &mut Ecs, s: Id, t: Id)
where
    R: Relation<Sources: Directed>,
{
    let Ok(r) = ecs.ids.get(s) else { return };
    let Some(tgt) = Tgt::<R>::find::<R>(ecs, r, t) else { return };
    let tpos = Src::<R>::remove::<R>(ecs, t, tgt.pos);
    Tgt::<R>::remove::<R>(ecs, s, tpos);
}
