#[macro_export]
macro_rules! schema {
    ($(#[$outer:meta])* $vis:vis component $name:ident; $($rest:tt)*) => {
        #[allow(non_upper_case_globals)]
        $(#[$outer])*
        $vis static $name: $crate::ComponentKey<()> =
            $crate::ComponentKey::new(concat!(module_path!(), "::", stringify!($name)));
        const _: () = {
            #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
            #[linkme(crate = $crate::__linkme)]
            static ENTRY: &'static $crate::UntypedKey = $name.untyped();
        };
        $crate::schema!{$($rest)*}
    };

    ($(#[$outer:meta])* $vis: vis component $name: ident: $ty: ty; $($rest: tt)*) => {
        #[allow(non_upper_case_globals)]
        $(#[$outer])*
        $vis static $name: $crate::ComponentKey<$ty> =
            $crate::ComponentKey::new(concat!(module_path!(), "::", stringify!($name)));
        const _: () = {
            #[$crate::__linkme::distributed_slice($crate::STATIC_COMPONENTS)]
            #[linkme(crate = $crate::__linkme)]
            static ENTRY: &'static $crate::UntypedKey = $name.untyped();
        };
        $crate::schema!{$($rest)*}
    };

    ($(#[$outer:meta])* $vis:vis relation $name:ident; $($rest:tt)*) => {
        #[allow(non_upper_case_globals)]
         $(#[$outer])*
        $vis static $name: $crate::RelationKey<()> =
            $crate::RelationKey::new(
                concat!(module_path!(), "::", stringify!($name))
            );
        const _: () = {
            #[$crate::__linkme::distributed_slice($crate::STATIC_RELATIONS)]
            #[linkme(crate = $crate::__linkme)]
            static ENTRY: &'static $crate::UntypedKey = $name.untyped();
        };
        $crate::schema! { $($rest)* }
    };

    ($(#[$outer:meta])* $vis:vis relation $name:ident: $ty:ty; $($rest:tt)*) => {
        #[allow(non_upper_case_globals)]
        $(#[$outer])*
        $vis static $name: $crate::RelationKey<$ty> =
            $crate::RelationKey::new(
                concat!(module_path!(), "::", stringify!($name))
            );
        const _: () = {
            #[$crate::__linkme::distributed_slice($crate::STATIC_RELATIONS)]
            #[linkme(crate = $crate::__linkme)]
            static ENTRY: &'static $crate::UntypedKey = $name.untyped();
        };
        $crate::schema! { $($rest)* }
    };

    () => {};
}
