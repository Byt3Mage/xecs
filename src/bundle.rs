use std::ptr::NonNull;

use crate::{
    Component, ComponentKey, IdsExhausted,
    component::{ComponentId, signature::Signature},
    ecs::Ecs,
    id::{
        Id,
        allocator::{IdAllocator, IdRecord},
    },
    table::{self, Table},
    table_storage::{TableId, TableStorage, TableVec},
    type_meta::TypeIdMap,
};

/// A set of distinct columns, named by marker types. The table graph keys
/// its transitions on the set's `TypeId`.
///
/// A set whose keys repeat a column fails to compile wherever it is
/// inserted or removed.
pub trait ColumnSet: Copy + 'static {
    const KEYS: &'static [&'static ComponentKey];
}

/// A column set users may insert and remove directly.
pub trait ComponentSet: ColumnSet {}

/// Fails compilation of every instantiation where `S` repeats a component,
/// reporting the repeated path.
#[inline(always)]
const fn assert_unique<S: ColumnSet>() {
    const {
        if let Some(path) = ComponentKey::find_repeated(S::KEYS) {
            panic!("{}", path);
        }
    }
}

impl<K: Component> ColumnSet for K {
    const KEYS: &'static [&'static ComponentKey] = &[K::KEY];
}

impl<K: Component> ComponentSet for K {}

/// Values for every component of `Set`.
///
/// # Safety
/// `write` calls `sink` exactly once per id of `Set::ids()`, in that order,
/// each time with a pointer to a valid value of that component's type that
/// is readable for the duration of the call.
pub unsafe trait Bundle: Copy {
    type Set: ColumnSet;

    /// Hands each payload to `sink`, in field order.
    fn write(self, sink: impl FnMut(NonNull<u8>));
}

/// A value of the static component `K`.
#[repr(transparent)]
pub struct Field<K: Component>(pub K::Data);

impl<K: Component> Field<K> {
    #[inline(always)]
    pub const fn new(value: K::Data) -> Self {
        Self(value)
    }
}

impl<K: Component> Clone for Field<K> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Component> Copy for Field<K> {}

// SAFETY: one payload, of `K::Value`, matching `K::ids()`.
unsafe impl<K: Component> Bundle for Field<K> {
    type Set = K;

    #[inline(always)]
    fn write(self, mut sink: impl FnMut(NonNull<u8>)) {
        sink(NonNull::from_ref(&self.0).cast());
    }
}

/// Implements `ComponentSet` and `Bundle` for tuples of two or more
/// components. One-element tuples are left unimplemented so a single
/// component is always spelled `K` / `Field<K>`.
macro_rules! impl_tuples {
    ($K0:ident, $($K:ident),+ $(,)?) => {
        impl<$K0: Component, $($K: Component),+> ColumnSet for ($K0, $($K,)+) {
            const KEYS: &'static [&'static ComponentKey] =
                &[<$K0 as Component>::KEY, $(<$K as Component>::KEY),+];
        }

        impl<$K0: Component, $($K: Component),+> ComponentSet for ($K0, $($K,)+) {}

        // SAFETY: one payload per field, of that field's `K::Value`, in the
        // same order as `Set::KEYS`.
        unsafe impl<$K0: Component, $($K: Component),+> Bundle for (Field<$K0>, $(Field<$K>,)+) {
            type Set = ($K0, $($K,)+);

            #[inline(always)]
            fn write(self, mut sink: impl FnMut(NonNull<u8>)) {
                #[allow(non_snake_case)]
                let ($K0, $($K,)+) = self;
                sink(NonNull::from_ref(&$K0.0).cast());
                $(sink(NonNull::from_ref(&$K.0).cast());)+
            }
        }
    };
}

impl_tuples!(K0, K1);
impl_tuples!(K0, K1, K2);
impl_tuples!(K0, K1, K2, K3);
impl_tuples!(K0, K1, K2, K3, K4);
impl_tuples!(K0, K1, K2, K3, K4, K5);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9, K10);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9, K10, K11);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9, K10, K11, K12);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9, K10, K11, K12, K13);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9, K10, K11, K12, K13, K14);
impl_tuples!(K0, K1, K2, K3, K4, K5, K6, K7, K8, K9, K10, K11, K12, K13, K14, K15);

struct ColumnSetInfo {
    /// Sorted signature of the set's components.
    sig: Signature,
    /// `(component, field index)`, ascending by component.
    fields: Box<[(ComponentId, u32)]>,
}

impl ColumnSetInfo {
    fn from_set<S: ColumnSet>() -> Self {
        let mut fields: Box<_> = S::KEYS.iter().enumerate().map(|(i, k)| (k.id(), i as u32)).collect();
        fields.sort_unstable_by_key(|f| f.0);
        debug_assert!(
            fields.windows(2).all(|pair| pair[0].0 != pair[1].0),
            "component set repeats a component id"
        );
        let sig = Signature::from(fields.iter().map(|&(c, _)| c).collect::<Vec<_>>());
        Self { sig, fields }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Move {
    /// Column index on the source table.
    pub(crate) src: u32,
    /// Column index on the destination table.
    pub(crate) dst: u32,
}

/// The transition taken by inserting one component set into one table.
/// `dst` equal to the source table means the values overwrite in place.
pub(crate) struct InsertEdge {
    pub(crate) dst: TableId,
    /// Destination column of each field, in field order.
    pub(crate) writes: Box<[u32]>,
    /// Source columns carried across. Empty when in place.
    pub(crate) moves: Box<[Move]>,
}

/// The transition taken by removing one component set from one table.
/// `dst` equal to the source table means none of the set is present.
pub(crate) struct RemoveEdge {
    pub(crate) dst: TableId,
    /// Source columns the destination keeps.
    pub(crate) moves: Box<[Move]>,
}

pub(crate) struct BundleRegistry {
    sets: TypeIdMap<ColumnSetInfo>,
}

impl BundleRegistry {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Self { sets: TypeIdMap::new() }
    }

    /// The info of `S`, built on first use.
    #[inline(always)]
    fn get_or_insert<S: ColumnSet>(&mut self) -> &ColumnSetInfo {
        self.sets.entry::<S>().or_insert_with(ColumnSetInfo::from_set::<S>)
    }
}

/// Column of each field in `sig`, in field order.
///
/// `sig` contains every field.
fn plan_in_place(sig: &Signature, fields: &[(ComponentId, u32)]) -> Box<[u32]> {
    let mut writes = vec![0; fields.len()].into_boxed_slice();
    let mut fields = fields.iter().peekable();

    for (col, &c) in sig.iter().enumerate() {
        if let Some(&(_, field)) = fields.next_if(|&&(f, _)| f == c) {
            writes[field as usize] = col as u32;
        }
    }

    writes
}

/// Fills each column of `dst_sig`, which is `src_sig ∪ fields`: from its
/// field when the set has that component, otherwise from the source.
fn plan_moved(src_sig: &Signature, dst_sig: &Signature, fields: &[(ComponentId, u32)]) -> (Box<[u32]>, Box<[Move]>) {
    let mut writes = vec![0; fields.len()].into_boxed_slice();
    let mut moves = Vec::with_capacity(src_sig.len());
    let mut fields = fields.iter().peekable();
    let mut source = src_sig.iter().enumerate().peekable();

    for (col, &c) in dst_sig.iter().enumerate() {
        let col = col as u32;
        let from_source = source.next_if(|&(_, &s)| s == c);

        match fields.next_if(|&&(f, _)| f == c) {
            Some(&(_, field)) => writes[field as usize] = col,
            None => {
                if let Some((src, _)) = from_source {
                    moves.push(Move { src: src as u32, dst: col });
                }
            }
        }
    }

    (writes, moves.into())
}

/// Moves of every component `dst_sig` keeps from `src_sig`.
///
/// `dst_sig` is a subset of `src_sig`.
fn plan_remove(src_sig: &Signature, dst_sig: &Signature) -> Box<[Move]> {
    let mut kept = dst_sig.iter().enumerate().peekable();
    src_sig
        .iter()
        .enumerate()
        .filter_map(|(src, &c)| {
            kept.next_if(|&(_, &d)| d == c)
                .map(|(dst, _)| Move { src: src as u32, dst: dst as u32 })
        })
        .collect()
}

/// Builds the edge taken by inserting `S` into the table `src`, creating
/// the destination table if needed.
fn build_insert<S: ColumnSet>(ecs: &mut Ecs, src: TableId) -> InsertEdge {
    let info = ecs.bundles.get_or_insert::<S>();
    let storage = &mut ecs.storage;

    match storage.tables[src].sig.try_union(&info.sig) {
        None => InsertEdge {
            dst: src,
            writes: plan_in_place(&storage.tables[src].sig, &info.fields),
            moves: Box::new([]),
        },
        Some(sig) => {
            let dst = storage.get_or_create(&mut ecs.components, sig);
            let (src_table, dst_table) = storage.tables.get_2(src, dst);
            let (writes, moves) = plan_moved(&src_table.sig, &dst_table.sig, &info.fields);
            InsertEdge { dst, writes, moves }
        }
    }
}

/// Builds the edge taken by removing `S` from the table `src`, creating the
/// destination table if needed.
fn build_remove<S: ColumnSet>(ecs: &mut Ecs, src: TableId) -> RemoveEdge {
    let info = ecs.bundles.get_or_insert::<S>();
    let storage = &mut ecs.storage;

    match storage.tables[src].sig.try_difference(&info.sig) {
        None => RemoveEdge { dst: src, moves: Box::new([]) },
        Some(sig) => {
            let dst = storage.get_or_create(&mut ecs.components, sig);
            let (src_table, dst_table) = storage.tables.get_2(src, dst);
            let moves = plan_remove(&src_table.sig, &dst_table.sig);
            RemoveEdge { dst, moves }
        }
    }
}

/// Inserts every component of `bundle` into `id`, overwriting values it
/// already has.
pub(crate) fn insert<B: Bundle>(ecs: &mut Ecs, r: IdRecord, bundle: B) {
    const { assert_unique::<B::Set>() };

    let TableStorage { inserts, tables, .. } = &mut ecs.storage;

    #[cold]
    #[inline(never)]
    fn insert_slow<B: Bundle>(ecs: &mut Ecs, r: IdRecord, bundle: B) {
        let edge = build_insert::<B::Set>(ecs, r.table);
        let edge = ecs.storage.inserts[r.table].entry::<B::Set>().or_insert(edge);
        unsafe { table::insert(&mut ecs.storage.tables, &mut ecs.ids, r, edge, bundle) };
    }

    // SAFETY: `r` is live, and the edge was built for `r.table` and `B::Set`.
    match inserts[r.table].get::<B::Set>() {
        Some(edge) => unsafe { table::insert(tables, &mut ecs.ids, r, edge, bundle) },
        None => insert_slow(ecs, r, bundle),
    }
}

/// Removes the components of `S` from `id`, ignoring any it does not have.
pub(crate) fn remove<S: ColumnSet>(ecs: &mut Ecs, r: IdRecord) {
    assert_unique::<S>();

    let TableStorage { removes, tables, .. } = &mut ecs.storage;

    #[cold]
    #[inline(never)]
    fn remove_slow<S: ColumnSet>(ecs: &mut Ecs, r: IdRecord) {
        let edge = build_remove::<S>(ecs, r.table);
        let edge = ecs.storage.removes[r.table].entry::<S>().or_insert(edge);
        unsafe { table::remove(&mut ecs.storage.tables, &mut ecs.ids, r, edge) };
    }

    // SAFETY: `r` is live, and the edge was built for `r.table`.
    match removes[r.table].get::<S>() {
        Some(edge) => unsafe { table::remove(tables, &mut ecs.ids, r, edge) },
        None => remove_slow::<S>(ecs, r),
    }
}

#[inline]
fn get_insert_edge<S, F, R>(ecs: &mut Ecs, table: TableId, f: F) -> R
where
    S: ColumnSet,
    F: FnOnce(&mut TableVec<Table>, &mut IdAllocator, &InsertEdge) -> R,
{
    #[cold]
    #[inline(never)]
    fn slow<S, F, R>(ecs: &mut Ecs, table: TableId, f: F) -> R
    where
        S: ColumnSet,
        F: FnOnce(&mut TableVec<Table>, &mut IdAllocator, &InsertEdge) -> R,
    {
        let edge = build_insert::<S>(ecs, table);
        let edge = ecs.storage.inserts[table].entry::<S>().or_insert(edge);
        f(&mut ecs.storage.tables, &mut ecs.ids, edge)
    }

    let TableStorage { tables, inserts, .. } = &mut ecs.storage;

    match inserts[table].get::<S>() {
        Some(edge) => f(tables, &mut ecs.ids, edge),
        None => slow::<S, F, R>(ecs, table, f),
    }
}

/// Creates one id holding `bundle`.
pub(crate) fn create<B: Bundle>(ecs: &mut Ecs, f: impl FnOnce(Id) -> B) -> Result<Id, IdsExhausted> {
    get_insert_edge::<B::Set, _, _>(ecs, TableStorage::ROOT, |tables, ids, edge| {
        ids.new_id(|id| {
            let table = &mut tables[edge.dst];
            table.data.reserve(1);
            // SAFETY:
            // - capacity for the row is reserved by the caller.
            // - the root table has no columns, so an edge out of it writes every
            //   column of its destination, one per field of `B::Set`.
            let row = unsafe { table::create_row(table, id, &edge.writes, f(id)) };
            IdRecord { table: edge.dst, row }
        })
    })
}

/// Creates `n` ids, each holding the bundle `f` returns for it. Nothing
/// is created unless all `n` ids can be allocated.
pub(crate) fn batch_create<B: Bundle>(
    ecs: &mut Ecs,
    n: u32,
    mut f: impl FnMut(u32, Id) -> B,
) -> Result<(), IdsExhausted> {
    ecs.ids.ensure_available(n)?;

    get_insert_edge::<B::Set, _, _>(ecs, TableStorage::ROOT, |tables, ids, edge| {
        let table = &mut tables[edge.dst];
        table.data.reserve(n as usize);
        for i in 0..n {
            ids.new_id(|id| {
                // SAFETY:
                // - capacity for the row is reserved by the caller.
                // - the root table has no columns, so an edge out of it writes every
                //   column of its destination, one per field of `B::Set`.
                let row = unsafe { table::create_row(table, id, &edge.writes, f(i, id)) };
                IdRecord { table: edge.dst, row }
            })?;
        }
        Ok(())
    })
}
