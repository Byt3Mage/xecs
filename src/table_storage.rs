use std::ops::{Index, IndexMut};

use ahash::AHashMap;

use crate::{
    bundle::{InsertEdge, RemoveEdge},
    component::{registry::ComponentRegistry, signature::Signature},
    macros::{declare_id, map},
    table::{Table, TableData},
    type_meta::TypeIdMap,
};

declare_id!(TableId, "T");

/// Values indexed by `TableId`.
pub(crate) struct TableVec<V>(Vec<V>);

impl<V> TableVec<V> {
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// ## Panics
    /// Panics if table ids `a` and `b` are equal or if either id is invalid.
    #[inline]
    pub(crate) fn get_2(&self, a: TableId, b: TableId) -> (&V, &V) {
        let (a, b) = (a.index(), b.index());
        validate_pair(a, b, self.0.len());
        // SAFETY: a and b are valid indices and not equal.
        let ptr = self.0.as_ptr();
        unsafe { (&*ptr.add(a), &*ptr.add(b)) }
    }

    /// ## Panics
    /// Panics if table ids `a` and `b` are equal or if either id is invalid.
    #[inline]
    pub(crate) fn get_2_mut(&mut self, a: TableId, b: TableId) -> (&mut V, &mut V) {
        let (a, b) = (a.index(), b.index());
        validate_pair(a, b, self.0.len());
        // SAFETY: a and b are valid indices and not equal.
        let ptr = self.0.as_mut_ptr();
        unsafe { (&mut *ptr.add(a), &mut *ptr.add(b)) }
    }
}

#[inline]
fn validate_pair(a: usize, b: usize, len: usize) {
    #[cold]
    #[inline(never)]
    fn fail(a: usize, b: usize, len: usize) -> ! {
        if a == b {
            panic!("table ids are equal (id = {a})");
        }
        if a >= len {
            panic!("table id {a} is out of bounds (len = {len})");
        }
        panic!("table id {b} is out of bounds (len = {len})");
    }

    if (a == b) || (a >= len) || (b >= len) {
        fail(a, b, len);
    }
}

impl<V> Index<TableId> for TableVec<V> {
    type Output = V;

    #[inline(always)]
    fn index(&self, index: TableId) -> &Self::Output {
        &self.0[index.index()]
    }
}

impl<V> IndexMut<TableId> for TableVec<V> {
    #[inline(always)]
    fn index_mut(&mut self, index: TableId) -> &mut Self::Output {
        &mut self.0[index.index()]
    }
}

pub(crate) struct TableStorage {
    pub(crate) tables: TableVec<Table>,
    /// Insert edges out of each table, keyed by component set. Parallel to `tables`.
    pub(crate) inserts: TableVec<TypeIdMap<InsertEdge>>,
    /// Remove edges out of each table, keyed by component set. Parallel to `tables`.
    pub(crate) removes: TableVec<TypeIdMap<RemoveEdge>>,

    sig_to_id: AHashMap<Signature, TableId>,
}

impl TableStorage {
    /// The table with no components, where every new id starts.
    pub const ROOT: TableId = TableId::new(0).unwrap();

    pub(crate) fn new() -> Self {
        let sig = Signature::from([]);
        Self {
            tables: TableVec(vec![Table {
                sig: sig.clone(),
                data: TableData::new(Box::new([])),
            }]),
            inserts: TableVec(vec![TypeIdMap::new()]),
            removes: TableVec(vec![TypeIdMap::new()]),
            sig_to_id: map![sig => Self::ROOT],
        }
    }

    /// The table with signature `sig`, creating it if it doesn't exist.
    pub(crate) fn get_or_create(&mut self, components: &mut ComponentRegistry, sig: Signature) -> TableId {
        *self.sig_to_id.entry(sig).or_insert_with_key(|sig| {
            let id = TableId::new(self.tables.len() as u32).expect("too many tables");
            self.tables.0.push(Table::create(id, components, sig.clone()));
            self.inserts.0.push(TypeIdMap::new());
            self.removes.0.push(TypeIdMap::new());
            id
        })
    }

    #[inline]
    pub(crate) fn ids(&self, from: usize) -> impl Iterator<Item = TableId> {
        // SAFETY: All ids up to tables.len() are non-max.
        (from..self.tables.len()).map(|id| unsafe { TableId::new_unchecked(id as u32) })
    }
}
