use std::{
    borrow::Borrow,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicU32, Ordering},
    },
};

use crate::{
    ecs::Ecs,
    id::allocator::IdRecord,
    macros::declare_id,
    table_storage::TableId,
    type_meta::{HasMeta, TypeMeta},
};

pub mod registry;
pub mod signature;

/// Reserved: `C#<n>` addresses a component by id in string queries, and
/// is the display form of an unnamed component.
pub(super) const PREFIX: &str = "C";

#[derive(Clone)]
pub enum Path {
    Static(&'static str),
    Shared(Arc<str>),
}

impl Path {
    #[inline]
    pub fn as_str(&self) -> &str {
        match self {
            Path::Static(s) => s,
            Path::Shared(s) => s,
        }
    }
}

impl std::ops::Deref for Path {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Path {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for Path {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Path {}

impl std::hash::Hash for Path {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl std::fmt::Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.as_str(), f)
    }
}

impl std::fmt::Debug for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.as_str(), f)
    }
}

impl From<&'static str> for Path {
    #[inline]
    fn from(s: &'static str) -> Self {
        Path::Static(s)
    }
}

impl From<String> for Path {
    #[inline]
    fn from(s: String) -> Self {
        Path::Shared(s.into())
    }
}

impl From<Arc<str>> for Path {
    #[inline]
    fn from(s: Arc<str>) -> Self {
        Path::Shared(s)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ComponentRegisterError {
    #[error("component path `{0}` is already registered")]
    DuplicatePath(Path),
    #[error("component path `{0}` uses the reserved `{PREFIX}` prefix")]
    ReservedPrefix(Path),
}

declare_id!(ComponentId, PREFIX);

const NULL_KEY: u32 = u32::MAX;

/// The id slot of one static component, assigned by `static_components`.
#[derive(Debug)]
#[repr(transparent)]
pub struct KeySlot(AtomicU32);

impl KeySlot {
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        Self(AtomicU32::new(NULL_KEY))
    }

    #[inline(always)]
    fn load(&self) -> u32 {
        self.0.load(Ordering::Relaxed)
    }

    #[inline(always)]
    fn store(&self, slot: u32) {
        self.0.store(slot, Ordering::Relaxed);
    }
}

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
        let mut slot = self.slot.load();
        if slot == NULL_KEY {
            slot = self.init_slot();
        }
        // SAFETY: `init_slot` only returns slots below `NULL_KEY`.
        unsafe { ComponentId::new_unchecked(slot) }
    }

    #[cold]
    #[inline(never)]
    fn init_slot(&self) -> u32 {
        static_components();
        let slot = self.slot.load();
        assert!(
            slot != NULL_KEY,
            "component key `{}` is not in STATIC_COMPONENTS",
            self.path
        );
        slot
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

/// A component declared with `schema!`, named by its own type.
///
/// # Safety
/// `KEY.meta()` describes `Value`.
pub unsafe trait Component: Copy + 'static {
    type Value: HasMeta;
    const KEY: &'static ComponentKey;

    #[inline(always)]
    fn id() -> ComponentId {
        Self::KEY.id()
    }
}

#[linkme::distributed_slice]
pub static STATIC_COMPONENTS: [&'static ComponentKey];

pub fn static_components() -> &'static [&'static ComponentKey] {
    static SORTED: LazyLock<Box<[&'static ComponentKey]>> = LazyLock::new(|| {
        assert!(STATIC_COMPONENTS.len() < NULL_KEY as usize, "too many component keys");

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

#[derive(Debug, Clone)]
pub struct ComponentConfig {
    pub path: Option<Path>,
    pub meta: TypeMeta,
}

impl ComponentConfig {
    pub fn new() -> Self {
        Self { path: None, meta: TypeMeta::of::<()>() }
    }

    pub fn named(path: impl Into<Path>) -> Self {
        Self {
            path: Some(path.into()),
            meta: TypeMeta::of::<()>(),
        }
    }

    pub fn path(mut self, path: impl Into<Path>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn meta(mut self, meta: TypeMeta) -> Self {
        self.meta = meta;
        self
    }
}

pub struct ComponentInfo {
    pub path: Path,
    pub meta: TypeMeta,
    pub tables: Vec<TableId>,
}

impl ComponentInfo {
    #[inline]
    pub(crate) fn new(path: Path, meta: TypeMeta) -> Self {
        Self { path, meta, tables: vec![] }
    }

    pub(crate) fn insert_table(&mut self, table: TableId) {
        if let Err(pos) = self.tables.binary_search(&table) {
            self.tables.insert(pos, table);
        }
    }
}

#[inline(always)]
pub(crate) unsafe fn get<T>(ecs: &Ecs, r: IdRecord, comp: ComponentId) -> Option<&T> {
    Some(unsafe { ecs.storage.tables[r.table].column_of(comp)?.row(r.row).cast().as_ref() })
}

#[inline(always)]
pub(crate) unsafe fn get_mut<T>(ecs: &mut Ecs, r: IdRecord, comp: ComponentId) -> Option<&mut T> {
    Some(unsafe { ecs.storage.tables[r.table].column_of(comp)?.row(r.row).cast().as_mut() })
}
