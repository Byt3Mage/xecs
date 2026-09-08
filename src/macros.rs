#[macro_export]
macro_rules! schema {
    ($(#[$outer:meta])* $vis:vis component $name:ident; $($rest:tt)*) => {
        $crate::schema!{$(#[$outer])* $vis component $name: (); $($rest)*}
    };

    ($(#[$outer:meta])* $vis: vis component $name: ident: $ty: ty; $($rest: tt)*) => {
        $(#[$outer])*
        #[allow(non_upper_case_globals)]
        $vis static $name: $crate::ComponentKey<$ty> = {
            #[doc(hidden)]
            static KEY: $crate::UntypedKey = $crate::UntypedKey::new(concat!(module_path!(), "::", stringify!($name)));

            #[doc(hidden)]
            #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
            #[linkme(crate = $crate::__linkme)]
            static ENTRY: &'static xecs::UntypedKey = &KEY;

            $crate::ComponentKey::new(&KEY)
        };
        $crate::schema!{$($rest)*}
    };
    () => {};
}

macro_rules! map {
    () => {{ ahash::AHashMap::new() }};
}

pub(crate) use map;

macro_rules! declare_id {
    ($name: ident, $prefix: literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(transparent)]
        pub struct $name(u32);

        impl $name {
            pub const NULL: Self = Self(u32::MAX);

            #[allow(dead_code)]
            #[inline]
            pub const fn from_raw(raw: u32) -> Self {
                Self(raw)
            }

            #[allow(dead_code)]
            #[inline]
            pub const fn raw(self) -> u32 {
                self.0
            }

            #[inline(always)]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                if *self == Self::NULL {
                    write!(f, "{}#NULL", $prefix)
                } else {
                    write!(f, "{}#{}", $prefix, self.0)
                }
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

macro_rules! impl_registry {
    ($registry: ty [$index: ty] -> $output: ty) => {
        impl std::ops::IndexMut<$index> for $registry {
            #[inline]
            fn index_mut(&mut self, index: $index) -> &mut Self::Output {
                &mut self.infos[index.index()]
            }
        }

        impl std::ops::Index<$index> for $registry {
            type Output = $output;

            #[inline]
            fn index(&self, index: $index) -> &Self::Output {
                &self.infos[index.index()]
            }
        }
    };
}

pub(crate) use impl_registry;
