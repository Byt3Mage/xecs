use crate::{Ecs, query::physical::PhysicalPlan};

/// One worker's unit of driver iteration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Chunk {
    pub(crate) table: u32,
    pub(crate) start: u32,
    pub(crate) len: u32,
}

pub struct ParallelConfig {
    chunk_size: Option<u32>,
}
/// Split the driver's matched tables into work units.
///
/// Sizing targets several chunks per worker rather than one: with
/// follows, per-row cost varies with fan size, so equal row counts are
/// not equal work. Over-decomposing lets a worker that drew cheap rows
/// take another chunk instead of idling.
///
/// Weighting chunks by fan size up front would balance better, but
/// measuring fan sizes means reading the adjacency for every driver row
/// — most of the cost of just walking it.
pub(crate) fn chunks(ecs: &Ecs, plan: &PhysicalPlan, cfg: &ParallelConfig, workers: usize) -> Box<[Chunk]> {
    const MIN: u32 = 64;
    const MAX: u32 = 4096;
    const PER_WORKER: usize = 4;

    let total: u32 = plan.tables.iter().map(|mt| ecs.tables[mt.id].num_rows()).sum();

    if total == 0 {
        return Box::new([]);
    }

    let size = cfg.chunk_size.unwrap_or_else(|| {
        let target = total as usize / (workers * PER_WORKER).max(1);
        (target as u32).clamp(MIN, MAX)
    });

    let mut out = Vec::with_capacity(total.div_ceil(size) as usize);

    for (i, mt) in plan.tables.iter().enumerate() {
        let rows = ecs.tables[mt.id].num_rows();
        let mut start = 0;
        while start < rows {
            let len = size.min(rows - start);
            out.push(Chunk { table: i as u32, start, len });
            start += len;
        }
    }

    out.into_boxed_slice()
}
