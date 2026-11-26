use std::sync::LazyLock;

use crate::{
    id::allocator::IdRecord,
    macros::declare_id,
    memory::RowLayout,
    table::Table,
    table_storage::{TableId, TableVec},
    type_meta::{HasMeta, TypeMeta},
    utils::KeySlot,
};

pub mod registry;
pub mod signature;

declare_id!(ComponentId, "C");

#[derive(Debug)]
pub struct ComponentKey {
    slot: &'static KeySlot,
    path: &'static str,
    meta: &'static TypeMeta,
}

impl ComponentKey {
    #[inline]
    pub const fn new(slot: &'static KeySlot, path: &'static str, meta: &'static TypeMeta) -> Self {
        Self { slot, path, meta }
    }

    /// The component's id, identical in every world.
    ///
    /// # Panics
    /// Panics if this key is not in `STATIC_COMPONENTS`.
    #[inline(always)]
    pub fn id(&self) -> ComponentId {
        match ComponentId::from_raw(self.slot()) {
            Some(c) => c,
            None => self.init_id(),
        }
    }

    #[cold]
    #[inline(never)]
    fn init_id(&self) -> ComponentId {
        static_components();
        match ComponentId::from_raw(self.slot()) {
            Some(c) => c,
            None => panic!("key `{}` is not in STATIC_COMPONENTS", self.path),
        }
    }

    #[inline]
    pub fn slot(&self) -> u32 {
        self.slot.load()
    }

    #[inline]
    pub const fn path(&self) -> &'static str {
        self.path
    }

    #[inline(always)]
    pub const fn meta(&self) -> &'static TypeMeta {
        self.meta
    }

    /// The path of the first component that occurs more than once in `keys`.
    ///
    /// `static_components` assigns one id per distinct path and rejects keys
    /// sharing a slot, so distinct paths are exactly distinct ids.
    pub const fn find_repeated(keys: &[&'static ComponentKey]) -> Option<&'static str> {
        #[inline]
        const fn str_eq(a: &str, b: &str) -> bool {
            let a = a.as_bytes();
            let b = b.as_bytes();

            if a.len() != b.len() {
                return false;
            }

            let mut i = 0;

            while i < a.len() {
                if a[i] != b[i] {
                    return false;
                }
                i += 1;
            }

            true
        }

        let mut i = 0;

        while i < keys.len() {
            let mut j = i + 1;
            while j < keys.len() {
                if str_eq(keys[i].path, keys[j].path) {
                    return Some(keys[i].path);
                }
                j += 1;
            }
            i += 1;
        }
        None
    }
}

impl std::fmt::Display for ComponentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.id(), self.path())
    }
}

#[linkme::distributed_slice]
pub static STATIC_COMPONENTS: [&'static ComponentKey];

pub fn static_components() -> &'static [&'static ComponentKey] {
    static SORTED: LazyLock<Box<[&'static ComponentKey]>> = LazyLock::new(|| {
        assert!(STATIC_COMPONENTS.len() < u32::MAX as usize, "too many component keys");

        let mut keys: Box<_> = STATIC_COMPONENTS.iter().copied().collect();

        keys.sort_unstable_by_key(|k| k.path);

        keys.windows(2).for_each(|pair| {
            assert_ne!(
                pair[0].path, pair[1].path,
                "duplicate component path `{}`",
                pair[0].path
            )
        });

        keys.iter().enumerate().for_each(|(i, k)| k.slot.store(i as u32));
        keys.iter().enumerate().for_each(|(i, k)| {
            assert!(
                k.slot.load() == (i as u32),
                "component key `{}` shares its slot with another key",
                k.path
            );
        });

        keys
    });
    &SORTED
}

/// A component declared with `schema!`, named by its own type.
///
/// # Safety
/// `KEY.meta()` describes `Data`.
pub unsafe trait Component: Copy + 'static {
    /// Data carried by the component.
    type Data: HasMeta;
    const KEY: &'static ComponentKey;
}

#[inline(always)]
pub fn id<C: Component>() -> ComponentId {
    C::KEY.id()
}

#[derive(Debug, Clone)]
pub struct ComponentDesc {
    pub layout: RowLayout,
}

impl ComponentDesc {
    #[inline(always)]
    pub fn new<T: HasMeta>() -> Self {
        Self { layout: RowLayout::of::<T>() }
    }

    #[inline(always)]
    pub fn from_meta(meta: &TypeMeta) -> Self {
        Self { layout: meta.layout }
    }
}

pub struct ComponentInfo {
    pub layout: RowLayout,
    pub tables: Vec<TableId>,
}

impl ComponentInfo {
    #[inline]
    pub(crate) fn new(layout: RowLayout) -> Self {
        Self { layout, tables: vec![] }
    }

    #[inline]
    pub(crate) fn from_meta(meta: &TypeMeta) -> Self {
        Self::new(meta.layout)
    }

    pub(crate) fn insert_table(&mut self, table: TableId) {
        if let Err(pos) = self.tables.binary_search(&table) {
            self.tables.insert(pos, table);
        }
    }
}

#[inline(always)]
pub(crate) unsafe fn get<T>(tables: &TableVec<Table>, r: IdRecord, comp: ComponentId) -> Option<&T> {
    Some(unsafe { tables[r.table].column_of(comp)?.row(r.row).cast().as_ref() })
}

#[inline(always)]
pub(crate) unsafe fn get_mut<T>(tables: &mut TableVec<Table>, r: IdRecord, comp: ComponentId) -> Option<&mut T> {
    Some(unsafe { tables[r.table].column_of(comp)?.row(r.row).cast().as_mut() })
}
