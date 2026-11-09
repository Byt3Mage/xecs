#[macro_export]
macro_rules! schema {
    ($(#[$outer:meta])* $vis:vis component $name:ident; $($rest:tt)*) => {
        $crate::schema!{$(#[$outer])* $vis component $name: (); $($rest)*}
    };

    ($(#[$outer:meta])* $vis:vis component $name:ident: $ty:ty; $($rest:tt)*) => {
        $(#[$outer])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
        $vis struct $name;

        // SAFETY: `KEY` was built from `<$ty as HasMeta>::META`.
        unsafe impl $crate::Component for $name {
            type Value = $ty;
            const KEY: &'static $crate::ComponentKey = {
                static SLOT: $crate::KeySlot = $crate::KeySlot::new();
                static KEY: $crate::ComponentKey = $crate::ComponentKey::new(
                    &SLOT,
                    concat!(module_path!(), "::", stringify!($name)),
                    <$ty as $crate::HasMeta>::META,
                );
                #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
                #[linkme(crate = $crate::__linkme)]
                static ENTRY: &'static $crate::ComponentKey = &KEY;
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
        pub struct $name(#[doc(hidden)] std::num::NonZeroU32);

        impl $name {
            #[allow(dead_code)]
            #[inline(always)]
            pub(crate) const fn new(raw: u32) -> Option<Self> {
                match ::core::num::NonZeroU32::new(raw ^ u32::MAX) {
                    Some(val) => Some(Self(val)),
                    None => None,
                }
            }

            /// # Safety
            /// Caller must ensure that `raw` is not equal to u32::MAX
            #[inline(always)]
            #[allow(dead_code)]
            pub(crate) const unsafe fn new_unchecked(raw: u32) -> Self {
                Self(unsafe { ::core::num::NonZeroU32::new_unchecked(raw ^ u32::MAX) })
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
