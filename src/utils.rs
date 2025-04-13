use std::mem::ManuallyDrop;

pub struct OnDrop<F: FnOnce()> {
    callback: ManuallyDrop<F>,
}

impl<F: FnOnce()> OnDrop<F> {
    /// Returns an object that will invoke the specified callback when dropped.
    pub fn new(callback: F) -> Self {
        Self {
            callback: ManuallyDrop::new(callback),
        }
    }
}

impl<F: FnOnce()> Drop for OnDrop<F> {
    fn drop(&mut self) {
        // SAFETY: We may move out of `self`, since this instance can never be observed after it's dropped.
        let callback = unsafe { ManuallyDrop::take(&mut self.callback) };
        callback();
    }
}

#[derive(Debug, Clone, Default)]
pub struct BitVec {
    storage: Vec<usize>,
    len: usize,
}

impl BitVec {
    const BITS: usize = usize::BITS as usize;

    /// Create a new empty BitVec
    pub fn new() -> Self {
        Self {
            storage: Vec::new(),
            len: 0,
        }
    }

    /// Create a BitVec with `len` bits (all initialized to `false`)
    pub fn with_capacity(len: usize) -> Self {
        let blocks = (len + (Self::BITS - 1)) / Self::BITS;

        Self {
            storage: vec![0; blocks],
            len,
        }
    }

    /// Creates a new BitVec with `len` bits, all initialized to `value`
    pub fn from_value(value: bool, len: usize) -> Self {
        let blocks = (len + Self::BITS - 1) / Self::BITS;
        let fill_value = if value { !0 } else { 0 };
        let mut storage = vec![fill_value; blocks];
        
        // Clear any excess bits in the last block
        if (len % Self::BITS) != 0 {
            if let Some(last) = storage.last_mut() {
                let mask = (1 << (len % Self::BITS)) - 1;
                *last &= mask;
            }
        }
        
        Self { storage, len }
    }

    /// Get the number of bits stored
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn push(&mut self, bit: bool) {
        let block = self.len / Self::BITS;

        if block >= self.storage.len() {
            self.storage.push(0);
        }

        if bit {
            self.storage[block] |= 1 << (self.len % Self::BITS);
        }

        self.len += 1;
    }

    /// Get the bit at `index` (returns `false` for out-of-bounds)
    pub fn get(&self, index: usize) -> bool {
        if index >= self.len {
            return false;
        }

        let block = index / Self::BITS;
        let offset = index % Self::BITS;
        (self.storage[block] & (1 << offset)) != 0
    }

    /// Set the bit at `index` to `true`
    pub fn set(&mut self, index: usize) {
        self.grow_if_needed(index);
        let block = index / Self::BITS;
        let offset = index % Self::BITS;
        self.storage[block] |= 1 << offset;
        self.len = self.len.max(index + 1);
    }

    /// Set the bit at `index` to `false`
    pub fn clear(&mut self, index: usize) {
        if index >= self.len {
            return;
        }
        let block = index / Self::BITS;
        let offset = index % Self::BITS;
        self.storage[block] &= !(1 << offset);
    }

    /// Flip the bit at `index`
    pub fn flip(&mut self, index: usize) {
        self.grow_if_needed(index);
        let block = index / Self::BITS;
        let offset = index % Self::BITS;
        self.storage[block] ^= 1 << offset;
        self.len = self.len.max(index + 1);
    }

    /// Count the number of bits set to `true`
    pub fn count_ones(&self) -> usize {
        self.storage.iter().map(|&block| block.count_ones() as usize).sum()
    }

    /// Resize the BitVec
    pub fn resize(&mut self, new_len: usize, value: bool) {
        let new_blocks = (new_len + Self::BITS - 1) / Self::BITS;
        self.storage.resize(new_blocks, if value { !0 } else { 0 });
        
        // Clear any excess bits if we're shrinking
        if new_len < self.len {
            let clear_block = new_len / Self::BITS;
            let clear_offset = new_len % Self::BITS;

            if clear_block < self.storage.len() {
                let mask = (1 << clear_offset) - 1;
                self.storage[clear_block] &= mask;
            }
        }
        
        self.len = new_len;
    }

    fn grow_if_needed(&mut self, index: usize) {
        let needed_blocks = (index + Self::BITS) / Self::BITS;

        if needed_blocks > self.storage.len() {
            self.storage.resize(needed_blocks, 0);
        }
    }
}

#[macro_export]
macro_rules! bits {
    // Initialize all bits to a specific value (bits![true; 100] or bits![false; 100])
    ($value:expr; $len:expr) => {{
        crate::utils::BitVec::from_value($value, $len) }
    };
    
    // Initialize from individual bits (bits![true, false, true])
    ($($bit:expr),* $(,)?) => {{
        let mut bv = $crate::utils::BitVec::new();
        $(bv.push($bit);)*
        bv
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let mut bv = BitVec::new();
        assert_eq!(bv.len(), 0);
        
        bv.set(5);
        assert!(bv.get(5));
        assert_eq!(bv.len(), 6);
        
        
        bv.clear(5);
        assert!(!bv.get(5));
        
        bv.flip(10);
        assert!(bv.get(10));
        bv.flip(10);
        assert!(!bv.get(10));
    }

    #[test]
    fn test_count_ones() {
        let mut bv = BitVec::new();
        bv.set(1);
        bv.set(3);
        bv.set(64);
        assert_eq!(bv.count_ones(), 3);
    }
}