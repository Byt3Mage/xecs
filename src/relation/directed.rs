use std::mem;

use crate::{
    Ecs, HasMeta, Id, RelationId,
    relation::{Direction, RelateError, Sources, Targets, edge::Edge, many::Many, target, target_mut},
};

pub(crate) fn relate<T: HasMeta>(ecs: &mut Ecs, rel: RelationId, s: Id, t: Id, d: T) -> Result<(), RelateError> {
    let r = ecs.ids.get(s)?;
    let info = &mut ecs.relations.infos[rel.index()];

    match &mut info.targets {
        Targets::One => {
            let tables = &mut ecs.storage.tables;
            let comp = info.component;

            if let Some(edge) = target_mut::<T>(tables, r, comp)
                && edge.target == t
            {
                edge.data = d;
                return Ok(());
            }

            ecs.ids.check_alive(t)?;

            if info.direction == Direction::Acyclic {
                let mut cur = t;
                loop {
                    if cur == s {
                        return Err(RelateError::Cycles { src: s, tgt: t });
                    }
                    let Some(next) = target::<T>(tables, ecs.ids[cur], comp) else { break };
                    cur = next.target;
                }
            }

            match target_mut::<T>(tables, r, comp) {
                Some(edge) => {
                    let prev = mem::replace(edge, Edge::new(t, d));

                    match &mut info.sources {
                        Sources::One(srcs) => srcs.remove(prev.target),
                        Sources::Many(srcs) => {
                            let many = srcs.get_mut(prev.target).expect("target has source");
                            let pos = many.iter().position(|&src| src == s).expect("target has source");

                            many.swap_remove(pos);

                            if many.is_empty() {
                                srcs.remove(prev.target);
                            }
                        }
                    }
                }
                None => { /* TODO: insert bundle */ }
            }
        }
        Targets::Many(many) => {
            if let Some(edges) = many.get_mut(s)
                && let Some(edge) = unsafe { edges.find_mut::<T>(t) }
            {
                edge.data = d;
                return Ok(());
            }

            ecs.ids.check_alive(t)?;

            let walk = &mut ecs.relations.walk;

            if info.direction == Direction::Acyclic {
                match &info.sources {
                    Sources::One(srcs) => {
                        let mut cur = s;
                        loop {
                            if cur == t {
                                return Err(RelateError::Cycles { src: s, tgt: t });
                            }
                            let Some(prev) = srcs.get(cur) else { break };
                            cur = *prev;
                        }
                    }
                    Sources::Many(_) => {
                        walk.begin();
                        walk.push(t);

                        while let Some(cur) = walk.pop() {
                            if cur == s {
                                return Err(RelateError::Cycles { src: s, tgt: t });
                            }

                            if walk.mark(cur)
                                && let Some(targets) = many.get(t)
                            {
                                targets.targets(info.edge.layout.size()).for_each(|t| walk.push(t));
                            }
                        }
                    }
                }
                return Err(RelateError::Cycles { src: s, tgt: t });
            }

            // TODO: bundle::insert(ecs, r, ComponentOf<R>);
            unsafe {
                many.get_or_insert(s, || Many::new(info.edge.layout))
                    .push(Edge { target: t, data: d })
            };
        }
    }

    let info = &mut ecs.relations[rel];

    match &mut info.sources {
        Sources::One(srcs) => srcs.insert(t, s),
        Sources::Many(srcs) => srcs.get_or_insert(t, Vec::new).push(s),
    }

    Ok(())
}

pub(crate) fn unrelate<T: HasMeta>(ecs: &mut Ecs, rel: RelationId, s: Id, t: Id) {
    let Ok(r) = ecs.ids.get(s) else { return };
    let info = &mut ecs.relations.infos[rel.index()];

    match &mut info.targets {
        Targets::One => {
            let tables = &mut ecs.storage.tables;
            let comp = info.component;
            let Some(tgt) = target_mut::<T>(tables, r, comp) else { return };

            if tgt.target != t {
                return;
            }
        }

        Targets::Many(many) => {
            let stride = info.edge.layout.size();
            let Some(edges) = many.get_mut(s) else { return };
            let Some(pos) = edges.position(stride, t) else { return };

            edges.swap_remove(stride, pos);

            if edges.is_empty() {
                // TODO dealloc
                many.remove(s);
            }
        }
    }

    //TODO: remove component

    match &mut info.sources {
        Sources::One(srcs) => srcs.remove(t),
        Sources::Many(srcs) => {
            let edges = srcs.get_mut(t).expect("target has source");
            let pos = edges.iter().position(|&src| src == s).expect("target has source");

            edges.swap_remove(pos);

            if edges.is_empty() {
                srcs.remove(t);
            }
        }
    }
}
