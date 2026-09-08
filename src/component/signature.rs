use std::rc::Rc;

use crate::component::id::ComponentId;

/// Sorted and deduped list of component ids in a [Table]
///
/// [Table]: crate::table::Table
#[derive(Debug, Hash, PartialEq, Eq)]
#[repr(transparent)]
pub struct Signature(Rc<[ComponentId]>);

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Clone for Signature {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T: Into<Vec<ComponentId>>> From<T> for Signature {
    fn from(value: T) -> Self {
        let mut vec = value.into();
        vec.sort_unstable();
        vec.dedup();
        Self(vec.into())
    }
}

impl std::ops::Deref for Signature {
    type Target = [ComponentId];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Signature {
    #[inline]
    pub fn find_id(&self, id: &ComponentId) -> Option<usize> {
        self.binary_search(id).ok()
    }

    #[inline]
    pub fn has_id(&self, id: &ComponentId) -> bool {
        self.binary_search(id).is_ok()
    }

    /// Creates a new sorted list from [Signature] and `with`
    ///
    /// Returns `None` if self already contains `with`.
    pub fn try_extend(&self, with: ComponentId) -> Option<Self> {
        match self.binary_search(&with) {
            Ok(_) => None,
            Err(pos) => Some({
                let mut new_sig = Vec::with_capacity(pos);
                new_sig.extend_from_slice(&self[..pos]);
                new_sig.push(with);
                new_sig.extend_from_slice(&self[pos..]);
                new_sig.into()
            }),
        }
    }

    /// Creates a new sorted list from [Signature] without `from`.
    ///
    /// Returns `None` if self doesn't contain `from`.
    pub fn try_shrink(&self, from: ComponentId) -> Option<Self> {
        match self.binary_search(&from) {
            Ok(pos) => Some({
                let mut new_list = Vec::from(self.as_ref());
                new_list.remove(pos);
                new_list.into()
            }),
            Err(_) => None,
        }
    }

    /// Sorted union with `other`, or `None` when `other` is contained in `self`
    /// and the union would therefore equal `self`.
    pub fn try_union(&self, other: &Signature) -> Option<Self> {
        let (mut a, mut b) = (0, 0);

        // Advance while every id in `other` is contained in self.
        // Until an uncontained id appears, the union is `self`.
        while b < other.len() {
            while a < self.len() && self[a] < other[b] {
                a += 1;
            }
            if a == self.len() || self[a] != other[b] {
                break;
            }
            a += 1;
            b += 1;
        }

        if b == other.len() {
            return None;
        }

        // `other[b]` is the first unheld id. Every element of `self[..a]` is
        // below it, and every element of `other[..b]` was matched within that
        // range, so the prefix is already the head of the union.
        //
        // Capacity is an upper bound: `other[b + 1..]` may contain held ids,
        // which dedup to one element each.
        let mut out = Vec::with_capacity(self.len() + (other.len() - b));
        out.extend_from_slice(&self[..a]);

        // Use a branchless merge. On a tie both cursors advance and one copy
        // is pushed, which performs the dedup.
        while a < self.len() && b < other.len() {
            let (x, y) = (self[a], other[b]);
            out.push(x.min(y));
            a += (x <= y) as usize;
            b += (y <= x) as usize;
        }

        // The loop exits when one input is exhausted; at most one tail runs.
        out.extend_from_slice(&self[a..]);
        out.extend_from_slice(&other[b..]);

        Some(Self(out.into())) // ids already sorted
    }

    pub fn try_difference(&self, other: &Signature) -> Option<Self> {
        let (mut a, mut b) = (0, 0);

        // Advance while no id in `other` is contained.
        // Until a contained id appears, the difference is `self`.
        while b < other.len() {
            while a < self.len() && self[a] < other[b] {
                a += 1;
            }
            if a == self.len() || self[a] == other[b] {
                break;
            }
            b += 1;
        }

        // `other` ran out with nothing matched, or `self` ran out
        // below everything left in `other`.
        if a == self.len() || b == other.len() {
            return None;
        }

        // `self[a] == other[b]` is the first id to drop. `self[..a]` is
        // entirely below it and survives. Capacity is an upper bound; the
        // result is a subsequence of `self`.
        let mut out = Vec::with_capacity(self.len() - 1);
        out.extend_from_slice(&self[..a]);
        a += 1;
        b += 1;

        while a < self.len() && b < other.len() {
            // Skip ids below the current one.
            // They're not contained, so they remove nothing.
            while b < other.len() && other[b] < self[a] {
                b += 1;
            }
            if b < other.len() && other[b] == self[a] {
                b += 1;
            } else {
                out.push(self[a]);
            }
            a += 1;
        }

        // `other` is exhausted; everything remaining in `self` survives.
        out.extend_from_slice(&self[a..]);

        Some(Self(out.into())) // ids already sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sig(ids: &[u32]) -> Signature {
        ids.iter()
            .copied()
            .map(ComponentId::from_raw)
            .collect::<Vec<_>>()
            .into()
    }

    fn raw(s: &Signature) -> Vec<u32> {
        s.iter().map(|c| c.raw()).collect()
    }

    /// Reference union: `None` when `other ⊆ self`.
    fn ref_union(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        if b.iter().all(|x| a.contains(x)) {
            return None;
        }
        let mut out: Vec<u32> = a.iter().chain(b).copied().collect();
        out.sort_unstable();
        out.dedup();
        Some(out)
    }

    /// Reference difference: `None` when `a ∩ b = ∅`.
    fn ref_difference(a: &[u32], b: &[u32]) -> Option<Vec<u32>> {
        if !a.iter().any(|x| b.contains(x)) {
            return None;
        }
        Some(a.iter().copied().filter(|x| !b.contains(x)).collect())
    }

    // ---- construction ----

    #[test]
    fn from_vec_sorts_and_dedups() {
        assert_eq!(raw(&sig(&[5, 2, 9, 2, 5])), vec![2, 5, 9]);
        assert_eq!(raw(&sig(&[])), Vec::<u32>::new());
        assert_eq!(raw(&sig(&[7])), vec![7]);
        assert_eq!(raw(&sig(&[3, 3, 3])), vec![3]);
    }

    #[test]
    fn equality_is_order_independent() {
        assert_eq!(sig(&[9, 2, 5]), sig(&[2, 5, 9]));
        assert_ne!(sig(&[2, 5]), sig(&[2, 5, 9]));
    }

    // ---- lookup ----

    #[test]
    fn find_and_has() {
        let s = sig(&[2, 5, 9]);
        assert_eq!(s.find_id(&ComponentId::from_raw(2)), Some(0));
        assert_eq!(s.find_id(&ComponentId::from_raw(9)), Some(2));
        assert_eq!(s.find_id(&ComponentId::from_raw(4)), None);
        assert!(s.has_id(&ComponentId::from_raw(5)));
        assert!(!s.has_id(&ComponentId::from_raw(0)));
        assert!(!sig(&[]).has_id(&ComponentId::from_raw(0)));
    }

    // ---- try_extend / try_shrink ----

    #[test]
    fn try_extend_positions() {
        let s = sig(&[2, 5, 9]);
        assert_eq!(raw(&s.try_extend(ComponentId::from_raw(0)).unwrap()), vec![0, 2, 5, 9]);
        assert_eq!(raw(&s.try_extend(ComponentId::from_raw(7)).unwrap()), vec![2, 5, 7, 9]);
        assert_eq!(
            raw(&s.try_extend(ComponentId::from_raw(11)).unwrap()),
            vec![2, 5, 9, 11]
        );
        assert!(s.try_extend(ComponentId::from_raw(5)).is_none());
        assert_eq!(raw(&sig(&[]).try_extend(ComponentId::from_raw(3)).unwrap()), vec![3]);
    }

    #[test]
    fn try_shrink_positions() {
        let s = sig(&[2, 5, 9]);
        assert_eq!(raw(&s.try_shrink(ComponentId::from_raw(2)).unwrap()), vec![5, 9]);
        assert_eq!(raw(&s.try_shrink(ComponentId::from_raw(5)).unwrap()), vec![2, 9]);
        assert_eq!(raw(&s.try_shrink(ComponentId::from_raw(9)).unwrap()), vec![2, 5]);
        assert!(s.try_shrink(ComponentId::from_raw(4)).is_none());
        assert!(sig(&[]).try_shrink(ComponentId::from_raw(0)).is_none());
        assert_eq!(
            raw(&sig(&[7]).try_shrink(ComponentId::from_raw(7)).unwrap()),
            Vec::<u32>::new()
        );
    }

    // ---- union ----

    #[test]
    fn union_contained_is_none() {
        let s = sig(&[2, 5, 9]);
        assert!(s.try_union(&sig(&[])).is_none());
        assert!(s.try_union(&sig(&[5])).is_none());
        assert!(s.try_union(&sig(&[2, 9])).is_none());
        assert!(s.try_union(&sig(&[2, 5, 9])).is_none());
        assert!(sig(&[]).try_union(&sig(&[])).is_none());
    }

    #[test]
    fn union_shapes() {
        let s = sig(&[2, 5, 9]);
        // new id below, between, above
        assert_eq!(raw(&s.try_union(&sig(&[0])).unwrap()), vec![0, 2, 5, 9]);
        assert_eq!(raw(&s.try_union(&sig(&[7])).unwrap()), vec![2, 5, 7, 9]);
        assert_eq!(raw(&s.try_union(&sig(&[11])).unwrap()), vec![2, 5, 9, 11]);
        // held and new mixed, divergence at first / middle / last position
        assert_eq!(raw(&s.try_union(&sig(&[1, 5])).unwrap()), vec![1, 2, 5, 9]);
        assert_eq!(raw(&s.try_union(&sig(&[2, 7])).unwrap()), vec![2, 5, 7, 9]);
        assert_eq!(raw(&s.try_union(&sig(&[5, 11])).unwrap()), vec![2, 5, 9, 11]);
        // held id after the first new one still dedups
        assert_eq!(raw(&s.try_union(&sig(&[7, 9])).unwrap()), vec![2, 5, 7, 9]);
        // fully disjoint, interleaved
        assert_eq!(raw(&s.try_union(&sig(&[1, 6, 10])).unwrap()), vec![1, 2, 5, 6, 9, 10]);
        // empty self
        assert_eq!(raw(&sig(&[]).try_union(&sig(&[3, 4])).unwrap()), vec![3, 4]);
    }

    #[test]
    fn union_exhausts_self_before_other() {
        // phase 1 runs off the end of `self`
        assert_eq!(raw(&sig(&[2]).try_union(&sig(&[2, 5, 9])).unwrap()), vec![2, 5, 9]);
        assert_eq!(raw(&sig(&[2, 5]).try_union(&sig(&[9])).unwrap()), vec![2, 5, 9]);
    }

    // ---- difference ----

    #[test]
    fn difference_disjoint_is_none() {
        let s = sig(&[2, 5, 9]);
        assert!(s.try_difference(&sig(&[])).is_none());
        assert!(s.try_difference(&sig(&[1])).is_none());
        assert!(s.try_difference(&sig(&[11])).is_none());
        assert!(s.try_difference(&sig(&[1, 6, 10])).is_none());
        assert!(sig(&[]).try_difference(&sig(&[2])).is_none());
        assert!(sig(&[]).try_difference(&sig(&[])).is_none());
    }

    #[test]
    fn difference_shapes() {
        let s = sig(&[2, 5, 9]);
        // remove first / middle / last
        assert_eq!(raw(&s.try_difference(&sig(&[2])).unwrap()), vec![5, 9]);
        assert_eq!(raw(&s.try_difference(&sig(&[5])).unwrap()), vec![2, 9]);
        assert_eq!(raw(&s.try_difference(&sig(&[9])).unwrap()), vec![2, 5]);
        // remove several
        assert_eq!(raw(&s.try_difference(&sig(&[2, 9])).unwrap()), vec![5]);
        assert_eq!(raw(&s.try_difference(&sig(&[2, 5, 9])).unwrap()), Vec::<u32>::new());
        // unheld ids ignored, before / between / after the match
        assert_eq!(raw(&s.try_difference(&sig(&[1, 5])).unwrap()), vec![2, 9]);
        assert_eq!(raw(&s.try_difference(&sig(&[2, 7])).unwrap()), vec![5, 9]);
        assert_eq!(raw(&s.try_difference(&sig(&[5, 11])).unwrap()), vec![2, 9]);
        assert_eq!(raw(&s.try_difference(&sig(&[0, 2, 4, 6, 8, 10])).unwrap()), vec![5, 9]);
        // superset
        assert_eq!(
            raw(&s.try_difference(&sig(&[0, 2, 5, 9, 11])).unwrap()),
            Vec::<u32>::new()
        );
    }

    #[test]
    fn difference_exhausts_other_mid_walk() {
        // `other` runs out after the first match; the rest of `self` survives
        assert_eq!(raw(&sig(&[2, 5, 9]).try_difference(&sig(&[2])).unwrap()), vec![5, 9]);
        // `other` runs out during the skip loop
        assert_eq!(raw(&sig(&[2, 5, 9]).try_difference(&sig(&[2, 6])).unwrap()), vec![5, 9]);
    }

    // ---- properties ----

    fn rand_sorted(seed: &mut u64, max: u32, len: usize) -> Vec<u32> {
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            v.push((*seed % max as u64) as u32);
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    #[test]
    fn union_matches_reference() {
        let mut seed = 0x243f_6a88_85a3_08d3u64;
        for i in 0..2000 {
            let a = rand_sorted(&mut seed, 12, i % 7);
            let b = rand_sorted(&mut seed, 12, i % 5);

            let got = sig(&a).try_union(&sig(&b)).map(|s| raw(&s));
            assert_eq!(got, ref_union(&a, &b), "union({a:?}, {b:?})");

            if let Some(ref out) = got {
                assert!(out.windows(2).all(|w| w[0] < w[1]), "unsorted: {out:?}");
                assert_ne!(*out, a, "None should have been returned for {a:?} ∪ {b:?}");
            }
        }
    }

    #[test]
    fn difference_matches_reference() {
        let mut seed = 0x1319_8a2e_0370_7344u64;
        for i in 0..2000 {
            let a = rand_sorted(&mut seed, 12, i % 7);
            let b = rand_sorted(&mut seed, 12, i % 5);

            let got = sig(&a).try_difference(&sig(&b)).map(|s| raw(&s));
            assert_eq!(got, ref_difference(&a, &b), "difference({a:?}, {b:?})");

            if let Some(ref out) = got {
                assert!(out.windows(2).all(|w| w[0] < w[1]), "unsorted: {out:?}");
                assert_ne!(*out, a, "None should have been returned for {a:?} \\ {b:?}");
            }
        }
    }

    #[test]
    fn round_trip() {
        let mut seed = 0xa409_3822_299f_31d0u64;
        for i in 0..2000 {
            let a = rand_sorted(&mut seed, 12, i % 7);
            let b = rand_sorted(&mut seed, 12, i % 5);
            let s = sig(&a);

            // Adding then removing the same ids yields `self` minus anything
            // it already held from `b`.
            if let Some(u) = s.try_union(&sig(&b)) {
                let back = u.try_difference(&sig(&b)).map(|d| raw(&d)).unwrap_or_else(|| raw(&u));
                let expected: Vec<u32> = a.iter().copied().filter(|x| !b.contains(x)).collect();
                assert_eq!(back, expected, "roundtrip {a:?} ∪ {b:?} \\ {b:?}");
            }
        }
    }
}
