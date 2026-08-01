use std::{
    alloc::{Layout, LayoutError},
    usize,
};

pub struct DynamicStructLayout {
    layout: Layout,
    offsets: Box<[usize]>,
}

impl DynamicStructLayout {
    pub fn repr_c(fields: &[Layout]) -> Result<DynamicStructLayout, LayoutError> {
        let mut layout = Layout::from_size_align(0, 1)?;
        let mut offsets = Vec::with_capacity(fields.len());
        for &field in fields {
            let (new_layout, offset) = layout.extend(field)?;
            layout = new_layout;
            offsets.push(offset);
        }
        Ok(Self {
            layout: layout.pad_to_align(),
            offsets: offsets.into_boxed_slice(),
        })
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn offsets(&self) -> &[usize] {
        &self.offsets
    }
}
