use std::alloc::Layout;

pub(crate) struct ByteBuffer(Box<[u8]>);

impl ByteBuffer {
    pub(crate) fn new(layout: Layout) -> Self {
        let ptr = unsafe { std::alloc::alloc(layout) };

        if ptr.is_null() {
            std::alloc::handle_alloc_error(layout);
        }

        // SAFETY: ptr is checked for null.
        unsafe { Self(Box::from_raw(std::slice::from_raw_parts_mut(ptr, layout.size()))) }
    }

    #[inline]
    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.0.as_ptr()
    }

    #[inline]
    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    #[inline]
    pub(crate) fn size(&self) -> usize {
        self.0.len()
    }
}