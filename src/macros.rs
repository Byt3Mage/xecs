#[macro_export]
macro_rules! schema {
    ($(#[$outer:meta])* $vis:vis component $name:ident$(: $data:ty)?; $($rest:tt)*) => {
        $(#[$outer])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        $vis struct $name;

        // SAFETY: `KEY` was built from `<$ty as HasMeta>::META`.
        unsafe impl $crate::Component for $name {
            #[allow(unused_parens)]
            type Data = ($($data)?);
            #[doc(hidden)]
            const KEY: &'static $crate::ComponentKey = {
                static SLOT: $crate::KeySlot = $crate::KeySlot::new();
                static KEY: $crate::ComponentKey = $crate::ComponentKey::new(
                    &SLOT,
                    concat!(module_path!(), "::", stringify!($name)),
                     #[allow(unused_parens)]
                    <($($data)?) as $crate::HasMeta>::META,
                );
                #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
                #[linkme(crate = $crate::__linkme)]
                static ENTRY: &'static $crate::ComponentKey = &KEY;
                &KEY
            };
        }

        $crate::schema!{$($rest)*}
    };

    ($(#[$outer:meta])* $vis:vis relation $name:ident {
            targets: $targets:ty,
            sources: $sources:ty,
            cycles: $cycles:ty,
            $(data: $data:ty $(,)?)?
    } $($rest:tt)*) => {
        $(#[$outer])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        $vis struct $name;

        // SAFETY: the edge key's meta is built from the forward type of
        // `$targets` and `$payload`, and `FNS` is `RelationFns::of::<$name>()`.
        unsafe impl $crate::Relation for $name {
            type Targets = $targets;
            type Sources = $sources;
            type Cycles = $cycles;
            #[allow(unused_parens)]
            type Data = ($($data)?);

            const KEY: &'static $crate::RelationKey = {
                static EDGE_SLOT: $crate::KeySlot = $crate::KeySlot::new();
                static EDGE: $crate::ComponentKey = $crate::ComponentKey::new(
                    &EDGE_SLOT,
                    concat!(module_path!(), "::", stringify!($name), "#edge"),
                    #[allow(unused_parens)]
                    <<$targets as $crate::Targets>::Row<$name> as $crate::HasMeta>::META,
                );
                #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
                #[linkme(crate = $crate::__linkme)]
                static EDGE_ENTRY: &'static $crate::ComponentKey = &EDGE;

                static VTABLE: $crate::RelationVtable = $crate::RelationVtable::of::<$name>();
                static SLOT: $crate::KeySlot = $crate::KeySlot::new();
                static KEY: $crate::RelationKey = $crate::RelationKey::new(
                    &SLOT,
                    concat!(module_path!(), "::", stringify!($name)),
                    &EDGE,
                    &VTABLE,
                );
                #[$crate::__linkme::distributed_slice($crate::STATIC_RELATIONS)]
                #[linkme(crate = $crate::__linkme)]
                static ENTRY: &'static $crate::RelationKey = &KEY;
                &KEY
            };
        }

        $crate::schema!{$($rest)*}
    };

    () => {};
}

/// Builds a static bundle: `bundle!(Hp: 100.0, Mana: 40.0)`.
#[macro_export]
macro_rules! bundle {
    ($K:ty$(: $value:expr)? $(,)?) => {
        #[allow(clippy::double_parens, unused_parens)]
        $crate::Field::<$K>::new(($($value)?))
    };

    ($($K:ty$(: $value:expr)?),+ $(,)?) => {{
        const _: () = {
           if let ::core::option::Option::Some(path) =
                $crate::ComponentKey::find_repeated(&[$(<$K as $crate::Component>::KEY),+])
            {
                   ::core::panic!("{}", path);
            }
        };
        #[allow(clippy::double_parens, unused_parens)]
        ($($crate::Field::<$K>::new(($($value)?)),)+)
    }};
}

macro_rules! map {
    () => {{ ahash::AHashMap::new() }};
    ($($key: expr => $value: expr),+ $(,)?) => {{
        let mut m = crate::macros::map![];
        $(m.insert($key, $value);)*
        m
    }};
}

pub(crate) use map;

macro_rules! declare_id {
    ($name: ident, $prefix: expr) => {
        /// A non-max 32-bit id for use in registries
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(#[doc(hidden)] $crate::utils::NonMaxU32);

        impl $name {
            #[allow(dead_code)]
            #[inline(always)]
            pub(crate) const fn new(n: $crate::utils::NonMaxU32) -> Self {
                Self(n)
            }

            #[allow(dead_code)]
            #[inline(always)]
            pub(crate) const fn from_raw(raw: u32) -> Option<Self> {
                match $crate::utils::NonMaxU32::new(raw) {
                    Some(val) => Some(Self(val)),
                    None => None,
                }
            }

            /// # Safety
            /// Caller must ensure that `raw` is not equal to u32::MAX
            #[inline(always)]
            #[allow(dead_code)]
            pub(crate) const unsafe fn new_unchecked(raw: u32) -> Self {
                Self(unsafe { $crate::utils::NonMaxU32::new_unchecked(raw) })
            }

            #[inline(always)]
            pub const fn raw(self) -> u32 {
                self.0.get() ^ u32::MAX
            }

            #[inline(always)]
            pub const fn index(self) -> usize {
                self.raw() as usize
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}#{}", $prefix, self.raw())
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(self, f)
            }
        }
    };
}

pub(crate) use declare_id;
