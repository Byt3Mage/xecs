pub struct ManyStore<T: Copy> {
    items: Vec<T>,
}

impl<T: Copy> Default for ManyStore<T> {
    fn default() -> Self {
        Self { items: Default::default() }
    }
}

impl<T: Copy> ManyStore<T> {
    pub(crate) fn new() -> Self {
        Self { items: Vec::new() }
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> u32 {
        self.items.len() as u32
    }

    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    #[inline(always)]
    pub(crate) fn next_slot(&self) -> u32 {
        self.len()
    }

    #[inline(always)]
    pub(crate) fn push(&mut self, value: T) -> u32 {
        let len = self.len();
        self.items.push(value);
        len
    }

    pub(crate) fn swap_remove(&mut self, pos: u32) -> Option<&T> {
        let index = pos as usize;
        self.items.swap_remove(index);
        if index == self.items.len() { None } else { Some(&self.items[index]) }
    }
}

impl<T: Copy> std::ops::Deref for ManyStore<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl<T: Copy> std::ops::DerefMut for ManyStore<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.items
    }
}

impl<T: Copy> std::ops::Index<u32> for ManyStore<T> {
    type Output = T;
    #[inline(always)]
    fn index(&self, i: u32) -> &T {
        &self.items[i as usize]
    }
}

impl<T: Copy> std::ops::IndexMut<u32> for ManyStore<T> {
    #[inline(always)]
    fn index_mut(&mut self, i: u32) -> &mut T {
        &mut self.items[i as usize]
    }
}
