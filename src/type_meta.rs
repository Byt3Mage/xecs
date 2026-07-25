use std::{alloc::Layout, any::TypeId, ptr::NonNull};

#[inline]
const fn dtor<T>() -> Option<unsafe fn(ptr: NonNull<u8>)> {
    if std::mem::needs_drop::<T>() {
        Some(|ptr: NonNull<u8>| unsafe { ptr.cast::<T>().drop_in_place() })
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TypeMeta {
    pub dtor: Option<unsafe fn(ptr: NonNull<u8>)>,
    pub dangling: NonNull<u8>,
    pub layout: Layout,
    pub type_id: TypeId,
    pub name: fn() -> &'static str,
}

impl TypeMeta {
    #[inline]
    pub const fn of<T: 'static>() -> Self {
        Self {
            dtor: dtor::<T>(),
            dangling: NonNull::<T>::dangling().cast(),
            layout: Layout::new::<T>(),
            type_id: TypeId::of::<T>(),
            name: std::any::type_name::<T>,
        }
    }

    #[inline(always)]
    pub fn is<T: 'static>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }

    #[inline(always)]
    pub const fn is_zst(&self) -> bool {
        self.layout.size() == 0
    }

    #[inline(always)]
    pub fn type_name(&self) -> &'static str {
        (self.name)()
    }

    #[inline(always)]
    pub fn assert_type<T: 'static>(&self) {
        assert!(
            self.is::<T>(),
            "type mismatch: expected {}, got {}",
            self.type_name(),
            std::any::type_name::<T>(),
        )
    }
}
