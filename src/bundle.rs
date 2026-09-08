use std::{cmp::Ordering, collections::hash_map::Entry, ptr::NonNull};

use ahash::AHashMap;

use crate::{
    component::{id::ComponentId, signature::Signature},
    ecs::Ecs,
    id::{Id, allocator::NotAlive},
    macros::{declare_id, impl_registry, map},
    table::{self},
    table_registry::{TableId, TableRegistry},
};

/// A set of component values to be written into a table.
///
/// # Safety
/// - `write` calls `sink` exactly `ids.len()` times, in the same order as
///   `ids`, each with a pointer to a valid value of that component's type.
/// - After `write` returns, the values are owned by the caller of `write`:
///   the implementor must not drop or reuse them.
pub unsafe trait Bundle {
    fn id(&self, bundles: &mut BundleRegistry) -> BundleId;

    /// Consumes the payloads, handing each to `sink` in `ids` order.
    ///
    /// # Safety
    /// May be called at most once per constructed bundle.
    unsafe fn write(self, sink: impl FnMut(NonNull<u8>));
}

unsafe impl<T> Bundle for (ComponentId, T) {
    fn id(&self, bundles: &mut BundleRegistry) -> BundleId {
        // TODO: optimize
        let sig = Signature::from([self.0]);
        bundles.intern(sig)
    }

    unsafe fn write(self, mut sink: impl FnMut(NonNull<u8>)) {
        sink(NonNull::from_ref(&self.1).cast());
        // The value has been moved into the table.
        std::mem::forget(self);
    }
}

declare_id!(BundleId, "B");

pub struct BundleInfo {
    sig: Signature,
    /// `table[src] -> table[dst]`.
    /// The next table reached by adding this bundle.
    insert: AHashMap<TableId, InsertEdge>,
    /// `table[dst] -> table[src]`.
    /// The next table reached by removing this bundle.
    remove: AHashMap<TableId, RemoveEdge>,
}

impl BundleInfo {
    fn new(sig: Signature) -> Self {
        Self { sig, insert: map![], remove: map![] }
    }
}

pub struct BundleRegistry {
    infos: Vec<BundleInfo>,
    id_by_signature: AHashMap<Signature, BundleId>,
}

impl_registry!(BundleRegistry[BundleId] -> BundleInfo);

impl BundleRegistry {
    pub(crate) fn new() -> Self {
        Self { id_by_signature: map![], infos: vec![] }
    }

    pub fn intern(&mut self, ids: Signature) -> BundleId {
        match self.id_by_signature.entry(ids) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                debug_assert!(self.infos.len() < (u32::MAX as usize), "too many bundles");
                let id = BundleId::from_raw(self.infos.len() as u32);
                self.infos.push(BundleInfo::new(e.key().clone()));
                e.insert(id);
                id
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Drop {
    /// Source column holding the value.
    pub(crate) col: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Move {
    /// Column index on the source table.
    pub(crate) src: u32,
    /// Column index on the destionation table.
    pub(crate) dst: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Write {
    /// Destination column to fill the value
    pub(crate) col: u32,
}

#[derive(Debug, Clone)]
pub enum InsertEdge {
    /// Every bundle component is already contained, so the signature is unchanged.
    /// Each payload replaces a value in the column it already occupies.
    InPlace { writes: Box<[Write]> },

    /// The bundle adds at least one component.
    Moved {
        dst: TableId,
        /// Source columns holding a value a bundle payload replaces.
        drops: Box<[Drop]>,
        /// Components carried across untouched.
        moves: Box<[Move]>,
        /// One destination column per bundle component, ascending by component
        /// id. Parallel to the bundle's fields.
        writes: Box<[Write]>,
    },
}

#[derive(Debug, Clone)]
pub enum RemoveEdge {
    Noop,
    Moved {
        dst: TableId,
        /// Components leaving the source table.
        drops: Box<[Drop]>,
        /// Source columns copied over to `dst` (ascending by component id).
        moves: Box<[Move]>,
    },
}

fn insert_in_place(tables: &TableRegistry, src_id: TableId, ids: &[ComponentId]) -> InsertEdge {
    let sig = &tables[src_id].sig;
    let mut writes = Vec::with_capacity(ids.len());
    let mut col = 0u32;

    for &id in ids {
        while sig[col as usize] != id {
            col += 1;
        }
        writes.push(Write { col });
        col += 1;
    }

    InsertEdge::InPlace { writes: writes.into() }
}

fn insert_moved(tables: &TableRegistry, src_id: TableId, dst_id: TableId, ids: &Signature) -> InsertEdge {
    let (src_table, dst_table) = tables.get_2(src_id, dst_id);
    let (src_len, ids_len) = (src_table.sig.len() as u32, ids.len() as u32);

    let (mut drops, mut moves, mut writes) = (vec![], vec![], Vec::with_capacity(ids.len()));
    let (mut src, mut dst, mut bun) = (0u32, 0u32, 0u32);

    while src < src_len && bun < ids_len {
        match src_table.sig[src as usize].cmp(&ids[bun as usize]) {
            Ordering::Less => {
                moves.push(Move { src, dst });
                src += 1;
            }
            Ordering::Greater => {
                writes.push(Write { col: dst });
                bun += 1;
            }
            Ordering::Equal => {
                drops.push(Drop { col: src });
                writes.push(Write { col: dst });
                src += 1;
                bun += 1;
            }
        }
        dst += 1;
    }

    while src < src_len {
        moves.push(Move { src, dst });
        src += 1;
        dst += 1;
    }

    while bun < ids_len {
        writes.push(Write { col: dst });
        bun += 1;
        dst += 1;
    }

    debug_assert_eq!(dst, dst_table.sig.len() as u32);

    InsertEdge::Moved {
        dst: dst_id,
        drops: drops.into(),
        moves: moves.into(),
        writes: writes.into(),
    }
}

fn remove_edge(tables: &TableRegistry, src_id: TableId, dst_id: TableId) -> RemoveEdge {
    let (src_table, dst_table) = tables.get_2(src_id, dst_id);
    let (src_len, dst_len) = (src_table.sig.len() as u32, dst_table.sig.len() as u32);
    let (mut drops, mut moves) = (vec![], vec![]);
    let (mut src, mut dst) = (0, 0);

    while src < src_len {
        let sid = src_table.sig[src as usize];
        if dst < dst_len && sid == dst_table.sig[dst as usize] {
            moves.push(Move { src, dst });
            dst += 1;
        } else {
            drops.push(Drop { col: src });
        }
        src += 1;
    }

    RemoveEdge::Moved {
        dst: dst_id,
        drops: drops.into(),
        moves: moves.into(),
    }
}

/// Ensures the insert edge for `(src, bundle)` exists.
pub(crate) fn ensure_insert_edge(ecs: &mut Ecs, src: TableId, bundle: BundleId) {
    let bundle = &mut ecs.bundles[bundle];
    if let Entry::Vacant(e) = bundle.insert.entry(src) {
        let tables = &mut ecs.tables;
        match tables[src].sig.try_union(&bundle.sig) {
            None => e.insert(insert_in_place(tables, src, &bundle.sig)),
            Some(sig) => {
                let dst = tables.get_or_create(&mut ecs.components, sig);
                e.insert(insert_moved(tables, src, dst, &bundle.sig))
            }
        };
    }
}

/// Ensures the remove edge for `(src, bundle)` exists.
pub(crate) fn ensure_remove_edge(ecs: &mut Ecs, src: TableId, bundle: BundleId) {
    let bundle = &mut ecs.bundles[bundle];
    if let Entry::Vacant(e) = bundle.remove.entry(src) {
        let tables = &mut ecs.tables;
        match tables[src].sig.try_difference(&bundle.sig) {
            None => e.insert(RemoveEdge::Noop),
            Some(sig) => {
                let dst = tables.get_or_create(&mut ecs.components, sig);
                e.insert(remove_edge(tables, src, dst))
            }
        };
    }
}

pub(crate) fn insert(ecs: &mut Ecs, id: Id, bundle: impl Bundle) -> Result<(), NotAlive> {
    let r = ecs.ids.get(id)?;
    let bid = bundle.id(&mut ecs.bundles);
    ensure_insert_edge(ecs, r.table, bid);
    let edge = &ecs.bundles[bid].insert[&r.table];
    // SAFETY: the edge was built for `r.table` and this bundle.
    unsafe { table::insert(&mut ecs.tables, &mut ecs.ids, id, r, edge, bundle) };
    Ok(())
}

/// Removes `bundle`'s components from `id`, ignoring any it does not have.
pub(crate) fn remove(ecs: &mut Ecs, id: Id, bundle: BundleId) -> Result<(), NotAlive> {
    let r = ecs.ids.get(id)?;
    ensure_remove_edge(ecs, r.table, bundle);
    let edge = &ecs.bundles[bundle].remove[&r.table];
    // SAFETY: the edge was built for `r.table` and this bundle.
    unsafe { table::remove(&mut ecs.tables, &mut ecs.ids, id, r, edge) };
    Ok(())
}
