use std::{
    alloc::{Layout, LayoutError},
    rc::Rc,
    usize,
};

pub struct DynamicStructLayout {
    layout: Layout,
    field_offsets: Rc<[usize]>,
}

impl DynamicStructLayout {
    pub fn new(fields: &[Layout]) -> Result<DynamicStructLayout, LayoutError> {
        let mut layout = Layout::from_size_align(0, 1)?;
        let mut offsets = vec![usize::MAX; fields.len()];

        for (i, &field) in fields.iter().enumerate() {
            let (new_layout, offset) = layout.extend(field)?;
            layout = new_layout;
            offsets[i] = offset;
        }

        Ok(Self {
            layout: layout.pad_to_align(),
            field_offsets: offsets.into(),
        })
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn offsets(&self) -> &[usize] {
        &self.field_offsets
    }
}
