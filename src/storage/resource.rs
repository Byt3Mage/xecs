use std::{alloc::Layout, ptr::NonNull};

use crate::type_meta::TypeMeta;

pub struct Resource {
    data: NonNull<u8>,
}

impl Resource {
    pub(crate) fn new<T: 'static>(val: T) -> Self {
        let data = if size_of::<T>() != 0 {
            unsafe {
                let layout = Layout::new::<T>();
                match NonNull::new(std::alloc::alloc(layout)) {
                    Some(p) => {
                        p.cast().write(val);
                        p
                    }
                    None => std::alloc::handle_alloc_error(layout),
                }
            }
        } else {
            NonNull::<T>::dangling().cast()
        };

        Self { data }
    }

    pub(crate) fn destroy(&mut self, meta: &TypeMeta) {
        unsafe {
            if let Some(dtor) = meta.dtor {
                dtor(self.data)
            }

            if !meta.is_zst() {
                std::alloc::dealloc(self.data.as_ptr(), meta.layout);
            }
        }
    }

    pub(crate) unsafe fn replace<T: 'static>(&mut self, value: T) -> T {
        unsafe { self.data.cast().replace(value) }
    }

    pub(crate) unsafe fn get<T: 'static>(&self) -> &T {
        unsafe { self.data.cast().as_ref() }
    }

    pub(crate) unsafe fn get_mut<T: 'static>(&self) -> &mut T {
        unsafe { self.data.cast().as_mut() }
    }
}
