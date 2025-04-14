use std::alloc::Layout;

/// Computes the layout of a `repr(C)` struct.
/// Returns (total_size, struct_alignment, field_offsets).
fn compute_c_layout(fields: &[Layout]) -> (usize, usize, Vec<usize>) {
    let mut max_align = 1;
    let mut current_offset = 0;
    let mut offsets = Vec::with_capacity(fields.len());

    for layout in fields {
        let size = layout.size();
        let align = layout.align();

        max_align = max_align.max(align);
        let padding = (align - (current_offset % align)) % align;
        offsets.push(current_offset + padding);
        current_offset += padding + size;
    }

    // Round up to `max_align`.
    let total_size = (current_offset + max_align - 1) / max_align * max_align;
    (total_size, max_align, offsets)
}