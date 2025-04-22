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
    (@from($world:expr, $id:expr)) => {{
        $crate::entity_view::EntityView::new(&mut $world, $id)
    }};
    
    (@use($view:expr)) => { $view };
    
    (@from($world:expr, $id:expr) $($methods:tt)*) => {{
        let receiver = view!(@from($world, $id));
        || -> Result<_, $crate::error::EcsError> { Ok($crate::chain_methods_impl!(receiver, $($methods)*))}()
    }};
    
    (@use($view:expr) $($methods:tt)*) => {{
        let receiver = view!(@use($view));
        || -> Result<_, $crate::error::EcsError> { Ok($crate::chain_methods_impl!(receiver, $($methods)*))}()
    }};
    
    (@use($view:expr) $($methods:tt)*) => {{
        let receiver = view!(@use($view));
        || -> Result<_, $crate::error::EcsError> { Ok($crate::chain_methods_impl!(receiver, $($methods)*))}()
    }};
}