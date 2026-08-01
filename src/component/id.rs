use std::{
    marker::PhantomData,
    sync::{
        LazyLock,
        atomic::{AtomicU32, Ordering},
    },
};

use crate::{TypeMeta, type_meta::HasMeta};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[repr(transparent)]
pub struct ComponentId(u32);

impl ComponentId {
    #[inline]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    #[inline(always)]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "C#{}", self.0)
    }
}

const NULL_KEY: u32 = u32::MAX;

#[derive(Debug)]
pub struct UntypedStaticId {
    slot: AtomicU32,
    path: &'static str,
}

impl UntypedStaticId {
    pub const fn new(path: &'static str) -> Self {
        Self { slot: AtomicU32::new(NULL_KEY), path }
    }

    /// Requires `boot()` to have completed. Guaranteed if this key was
    /// reached through a `World`, since `World::new` boots first.
    /// Requires `boot()` to have completed. Guaranteed if this key was
    /// reached through a `World`, since `World::new` boots first.
    #[inline]
    pub(crate) fn slot(&self) -> u32 {
        self.slot.load(Ordering::Relaxed)
    }

    #[inline]
    pub const fn path(&self) -> &'static str {
        self.path
    }
}

impl std::fmt::Display for UntypedStaticId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "S#{}({})", self.slot(), self.path())
    }
}

pub struct StaticId<T: HasMeta> {
    id: UntypedStaticId,
    marker: PhantomData<fn() -> T>,
}

impl<T: HasMeta> StaticId<T> {
    #[inline(always)]
    pub const fn new(path: &'static str) -> Self {
        Self {
            id: UntypedStaticId::new(path),
            marker: PhantomData,
        }
    }

    #[inline(always)]
    pub const fn untyped(&self) -> &UntypedStaticId {
        &self.id
    }

    #[inline(always)]
    pub(crate) fn slot(&self) -> u32 {
        self.id.slot()
    }

    #[inline(always)]
    pub const fn path(&self) -> &'static str {
        self.id.path()
    }

    #[inline(always)]
    pub const fn meta(&self) -> &'static TypeMeta {
        T::META
    }
}

impl<T: HasMeta> std::fmt::Display for StaticId<T> {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.untyped().fmt(f)
    }
}

/// A Rust type with an associated [StaticId]
pub trait TypedStaticId: HasMeta + Sized {
    fn id() -> &'static StaticId<Self>;
}

#[linkme::distributed_slice]
pub static STATIC_COMPONENTS: [&'static UntypedStaticId];

pub fn initialize_statics() -> usize {
    static STATIC_COUNT: LazyLock<usize> = LazyLock::new(|| {
        assert!(
            STATIC_COMPONENTS.len() < NULL_KEY as usize,
            "too many static components"
        );

        let mut ids = STATIC_COMPONENTS.iter().collect::<Vec<_>>();

        ids.sort_unstable_by_key(|k| k.path);

        for pair in ids.windows(2) {
            assert!(
                pair[0].path != pair[1].path,
                "duplicate component path `{}`",
                pair[0].path
            );
        }

        for (i, key) in ids.iter().enumerate() {
            key.slot.store(i as u32, Ordering::Relaxed);
        }

        ids.len()
    });
    *STATIC_COUNT
}

#[inline]
pub fn static_id_count() -> usize {
    initialize_statics()
}

#[macro_export]
macro_rules! components {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(
            #[allow(non_upper_case_globals)]
            pub static $name: $crate::StaticId<$ty> = $crate::StaticId::new(concat!(module_path!(), "::", stringify!($name)));
            const _: () = {
                #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
                #[linkme(crate = $crate::__linkme)]
                static ENTRY: &'static $crate::UntypedStaticId = $name.untyped();
            };
        )*
    };
}
