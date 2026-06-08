#[macro_export]
macro_rules! chain_methods_impl {
    // ? -> with type params
    ($receiver:expr, $func:ident::<$($param:ty),*>($($arg:expr),*)? $($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver.$func::<$($param),*>($($arg),*)?, $($rest)*)
    };

    // ? -> without type params
    ($receiver:expr, $func:ident($($arg:expr),*)? $($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver.$func($($arg),*)?,$($rest)*)
    };

    // expect -> with type params
    ($receiver:expr, $func:ident::<$($param:ty),*>($($arg:expr),*) | $msg:literal $($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver.$func::<$($param),*>($($arg),*).expect($msg), $($rest)*)
    };

    // expect -> without type params
    ($receiver:expr, $func:ident($($arg:expr),*) | $msg:literal $($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver.$func($($arg),*).expect($msg), $($rest)*)
    };

    // unwrap -> with type params
    ($receiver:expr, $func:ident::<$($param:ty),*>($($arg:expr),*) $($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver.$func::<$($param),*>($($arg),*).unwrap(),$($rest)*)
    };

    // unwrap -> without type params
    ($receiver:expr, $func:ident($($arg:expr),*) $($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver.$func($($arg),*).unwrap(), $($rest)*)
    };

    ($receiver:expr, .$($rest:tt)*) => {
        $crate::chain_methods_impl!($receiver, $($rest)*)
    };

    // Termination cases
    ($receiver:expr,) => { $receiver };
    ($receiver:expr)  => { $receiver };
}

#[macro_export]
macro_rules! view {
    (@from($ecs:expr, $id:expr)) => {{
        $crate::id::entity_view::EntityView::new(&mut $ecs, $id)
    }};

    (@use($view:expr)) => { $view };

    (@from($ecs:expr, $id:expr) $($methods:tt)*) => {{
        let receiver = view!(@from($ecs, $id));
        || -> Result<_, $crate::error::EcsError> { Ok($crate::chain_methods_impl!(receiver, $($methods)*))}()
    }};

    (@use($view:expr) $($methods:tt)*) => {{
        let receiver = view!(@use($view));
        || -> Result<_, $crate::error::EcsError> { Ok($crate::chain_methods_impl!(receiver, $($methods)*))}()
    }};
}

#[macro_export]
macro_rules! tuple_count {
    () => { 0 };
    ($head:ident) => { 1 };
    ($head:ident, $($tail:ident),*) => { 1 + tuple_count!($($tail),*) };
}
