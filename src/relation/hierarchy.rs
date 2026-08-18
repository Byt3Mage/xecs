use crate::{
    Id,
    data_structures::{Sparse, VecIdxU32},
    relation::storage::RelateError,
};

const NONE: u32 = u32::MAX;

/// Position of a node: which tree, and where inside it.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Location {
    tree: u32,
    slot: u32,
}

/// One connected tree, in DFS order.
///
/// Invariants, held by every mutation:
///     1. `parent[0] == NONE`, and `parent[i] < i` for every `i > 0`.
///     2. `i < exit[i] <= exit[parent[i]]`.
///     3. the subtree rooted at `i` is exactly `i..exit[i]`.
///
/// (1) makes propagation a flat forward loop: parent's output is
/// final before its child is reached. (3) makes a subtree a slice, which
/// is what transitive follows and despawn cascades read.
///
/// Payload invariant:
///
///     4. `rows[i]` is live iff `parent[i] != NONE`.
///
/// Payload belongs to the edge, and a root has no edge. So, row 0 is
/// never live. This allows extraction and insertion to move rows
/// without a placeholder value, and is what `Drop` relies on.
///
/// `parent` and `exit` are tree-local, so relocating a tree within
/// the forest rewrites no indices.
struct Tree {
    order: VecIdxU32<Id>,
    parent: VecIdxU32<u32>,
    exit: VecIdxU32<u32>,
}

impl Tree {
    fn new() -> Self {
        Self {
            order: VecIdxU32::new(),
            parent: VecIdxU32::new(),
            exit: VecIdxU32::new(),
        }
    }

    #[inline]
    fn len(&self) -> u32 {
        self.order.len()
    }

    #[inline]
    fn cap(&self) -> u32 {
        self.order.cap()
    }

    /// Tests if the `child` has a parent. A child has a parent when its
    /// parent entry is not `NONE`
    #[inline]
    fn has_parent(&self, child: u32) -> bool {
        self.parent[child] != NONE
    }

    #[inline]
    fn parent(&self, child: u32) -> Option<Id> {
        Some(self.parent[child]).filter(|&p| p != NONE).map(|p| self.order[p])
    }

    /// A node has children when its first child slot falls inside its
    /// own subtree range.
    #[inline]
    fn has_children(&self, parent: u32) -> bool {
        (parent + 1) < self.exit[parent]
    }

    /// First child of `parent`. The first is at `parent + 1`, and each
    /// subsequent sibling begins where the previous one's subtree ends.
    #[inline]
    fn first_child(&self, parent: u32) -> Option<u32> {
        Some(parent + 1).filter(|&c| c < self.exit[parent])
    }

    /// Each child's subtree is contiguous, so the next sibling begins
    /// where the previous one's subtree ends.
    #[inline]
    fn next_sibling(&self, parent: u32, child: u32) -> Option<u32> {
        Some(self.exit[child]).filter(|&n| n < self.exit[parent])
    }

    #[inline]
    fn subtree(&self, parent: u32) -> &[Id] {
        &self.order[parent..self.exit[parent]]
    }

    /// Adjust `exit` for `p` and every ancestor: the subtree they
    /// contain grew or shrank by `delta`.
    ///
    /// Only ancestors need this. Any other node either contains `p`
    /// (making it an ancestor) or is disjoint from it, in which case
    /// its range ends before the change.
    fn update_ancestors_exits(&mut self, p: u32, delta: i64) {
        let mut a = p;
        loop {
            self.exit[a] = (self.exit[a] as i64 + delta) as u32;
            match self.parent[a] {
                NONE => break,
                q => a = q,
            }
        }
    }

    /// Open `n` slots at `at`.
    ///
    /// Shape entries are placeholders and payload rows are left
    /// uninitialised. The caller writes both, because only the caller
    /// knows what is going in.
    ///
    /// Positions at or past `at` move up by `n`. Their `exit` always
    /// shifts, since an exit exceeds its own position and so is at
    /// least `at`. Their `parent` shifts only when it too lay at or
    /// past `at`; a parent below `at` did not move.
    fn open(&mut self, at: u32, n: u32) {
        let (a, k) = (at as usize, n as usize);
        self.order.splice(a..a, std::iter::repeat_n(Id::NULL, k));
        self.parent.splice(a..a, std::iter::repeat_n(NONE, k));
        self.exit.splice(a..a, std::iter::repeat_n(0, k));

        for q in (at + n)..self.len() {
            if self.parent[q] != NONE && self.parent[q] >= at {
                self.parent[q] += n;
            }
            self.exit[q] += n;
        }
    }

    /// Remove `[s, e)`.
    ///
    /// The caller has already moved the payload out and adjusted the
    /// ancestors' `exit`, so this only closes the hole.
    ///
    /// Positions past `e` move down by `n`, with the same asymmetry as
    /// `open`. A parent inside `[s, e)` is impossible: such a node
    /// would be a descendant of the node at `s`, and descendants are
    /// confined to `s..e`.
    fn close(&mut self, s: u32, e: u32) {
        let n = e - s;
        let len = self.len();

        for i in e..len {
            if self.parent[i] != NONE && self.parent[i] >= e {
                self.parent[i] -= n;
            }
            self.exit[i] -= n;
        }

        let (s, e) = (s as usize, e as usize);
        self.order.drain(s..e);
        self.parent.drain(s..e);
        self.exit.drain(s..e);
    }

    #[inline]
    pub fn propagate<I, O>(&mut self, input: &[I], output: &mut [O], mut f: impl FnMut(Option<&O>, &I) -> O) {
        let len = self.order.len();

        assert_eq!(input.len(), len as usize);
        assert_eq!(output.len(), len as usize);

        for i in 0..len {
            let (done, rest) = output.split_at_mut(i as usize);
            let p = self.parent[i];
            let parent = if p != NONE { Some(&done[p as usize]) } else { None };
            rest[0] = f(parent, &input[i as usize]);
        }
    }
}

/// Holds a subtree between leaving one tree and entering another.
///
/// Indices are subtree-local: the staged root is slot 0 with parent
/// `NONE`, so the fragment can be dropped into any tree at any offset
/// by adding the destination base.
///
/// Payload follows the same invariant as `Tree`: row 0 is never live.
/// The staged root's edge has ended by definition — it was severed to
/// get here — so its old value is dropped during staging and the new
/// one is written by whoever re-attaches it.
struct Scratch {
    order: VecIdxU32<Id>,
    parent: VecIdxU32<u32>,
    exit: VecIdxU32<u32>,
    /// Reused by `purge`, which lists children before detaching them.
    ids: VecIdxU32<Id>,
}

impl Scratch {
    fn new() -> Self {
        Self {
            order: VecIdxU32::new(),
            parent: VecIdxU32::new(),
            exit: VecIdxU32::new(),
            ids: VecIdxU32::new(),
        }
    }
}

/// A forest, stored as its own DFS order.
///
/// This is the whole storage for a `Topology::Hierarchy` relation.
/// There is no separate forward or reverse index. Both directions are
/// answered from these arrays, so the two can never disagree and no
/// synchronisation exists to be forgotten.
///
/// Trees are separate allocations rather than spans of one array. A
/// shared array would force every tree after a resized one to shift,
/// making a reparent O(forest). Separate trees bound every mutation by
/// the tree it touches, and let propagation run per tree in parallel
/// with no analysis needed.
pub struct Hierarchy {
    locate: Sparse<Location>,
    trees: VecIdxU32<Tree>,
    scratch: Scratch,
}

impl Hierarchy {
    pub(crate) fn new() -> Self {
        Self {
            trees: VecIdxU32::new(),
            locate: Sparse::new(),
            scratch: Scratch::new(),
        }
    }

    /// Stage a node that is not yet in the forest.
    fn stage_new(&mut self, id: Id) {
        debug_assert_eq!(self.scratch.order.len(), 0, "scratch was not consumed");

        self.scratch.order.push(id);
        self.scratch.parent.push(NONE);
        self.scratch.exit.push(1);
    }

    /// Stage `[s, e)` of tree `t`, rebasing indices so the extracted
    /// root becomes slot 0.
    ///
    /// The tree's shape is left untouched (handled by `close`) but
    /// its payload rows in the range are moved out and are dead
    /// afterwards.
    fn stage_subtree(&mut self, t: u32, s: u32, e: u32) {
        let tree = &self.trees[t];
        let scratch = &mut self.scratch;
        debug_assert_eq!(scratch.order.len(), 0, "scratch was not consumed");

        scratch.order.extend_from_slice(&tree.order[s..e]);

        // The staged root's parent lies outside the range and is
        // discarded. Every other parent is a descendant of `s`, so it
        // is inside the range and rebasing is a subtraction.
        scratch.parent.push(NONE);
        ((s + 1)..e).for_each(|q| scratch.parent.push(tree.parent[q] - s));
        (s..e).for_each(|q| scratch.exit.push(tree.exit[q] - s));
    }

    /// Attach the staged fragment under `p` in tree `t`, as its last
    /// child.
    ///
    /// Appending at `exit[p]` leaves every other subtree's range
    /// intact, so only `p`'s ancestors need adjusting.
    ///
    /// Leaves the fragment root's payload row uninitialized. The
    /// caller writes the new edge's value.
    fn attach_staged(&mut self, t: u32, p: u32) {
        let n = self.scratch.order.len();
        let at = self.trees[t].exit[p];

        self.trees[t].open(at, n);
        self.trees[t].update_ancestors_exits(p, n as i64);

        let tree = &mut self.trees[t];
        let scratch = &mut self.scratch;

        for k in 0..n {
            let q = at + k;
            tree.order[q] = scratch.order[k];
            tree.parent[q] = match scratch.parent[k] {
                NONE => p,
                r => at + r,
            };
            tree.exit[q] = at + scratch.exit[k];
        }

        scratch.order.clear();
        scratch.parent.clear();
        scratch.exit.clear();
    }

    /// Promote the staged fragment to a tree of its own.
    ///
    /// Its root keeps no forward edge, so row 0 is uninitialized.
    /// This is the payload invariant for a tree root.
    fn promote_staged(&mut self) -> u32 {
        let t = self.trees.len();

        let mut tree = Tree::new();

        tree.order.extend_from_slice(&self.scratch.order);
        tree.parent.extend_from_slice(&self.scratch.parent);
        tree.exit.extend_from_slice(&self.scratch.exit);

        self.scratch.order.clear();
        self.scratch.parent.clear();
        self.scratch.exit.clear();

        self.trees.push(tree);
        self.reindex(t);
        t
    }

    /// Rewrite `locate` for a whole tree.
    ///
    /// Operation is O(tree) in the same order as the mutation
    /// that preceded it.
    fn reindex(&mut self, tree: u32) {
        let t = &self.trees[tree];
        for slot in 0..t.len() {
            let id = t.order[slot];
            self.locate.set(id, Location { tree, slot });
        }
    }

    /// Discard a tree structurally, without touching `locate` for its
    /// own nodes. The caller owns those, having either restaged them
    /// or being about to evict them.
    ///
    /// `swap_remove` moves the last tree into the hole, so *that*
    /// tree's nodes are reindexed. No other tree is affected: `parent`
    /// and `exit` are tree-local, so nothing outside references a tree
    /// by index.
    fn remove_tree(&mut self, t: u32) {
        self.trees.swap_remove(t);
        if t < self.trees.len() {
            self.reindex(t);
        }
    }

    /// A node with neither parent nor children holds no edge, so it is
    /// not part of the relation and is removed.
    fn prune_if_isolated(&mut self, id: Id) {
        if let Some(&l) = self.locate.get(id)
            && self.trees[l.tree].len() == 1
        {
            self.locate.remove(id);
            self.remove_tree(l.tree);
        }
    }

    /// Admit a node as a singleton tree, or report where it already is.
    ///
    /// Its payload row stays uninitialised: a lone node is a tree root,
    /// and roots hold no edge.
    fn ensure_tree(&mut self, id: Id) -> Location {
        if let Some(&l) = self.locate.get(id) {
            return l;
        }

        let t = self.trees.len();
        let mut tree = Tree::new();

        tree.order.push(id);
        tree.parent.push(NONE);
        tree.exit.push(1);
        self.trees.push(tree);

        let l = Location { tree: t, slot: 0 };
        self.locate.set(id, l);
        l
    }

    /// Lift `child`'s subtree into scratch and repair the tree it left.
    ///
    /// On return the fragment is staged and `child`'s old tree is
    /// consistent, but the staged nodes' `locate` entries are stale —
    /// the caller re-homes them by attaching, promoting, or evicting.
    fn detach(&mut self, child: Id) {
        let l = self.locate[child];
        let (t, s) = (l.tree, l.slot);
        let e = self.trees[t].exit[s];

        self.stage_subtree(t, s, e);

        if s == 0 {
            // The staged range was the whole tree: a root's subtree
            // ends at `len`. `close` empties the shape arrays, so
            // `Tree::drop` finds nothing live since the payload is in
            // scratch.
            debug_assert_eq!(e, self.trees[t].len());
            self.trees[t].close(s, e);
            self.remove_tree(t);
            return;
        }

        let p = self.trees[t].parent[s];
        let old_parent = self.trees[t].order[p];

        // Ancestors first: `close` renumbers, and `bump_ancestors`
        // walks pre-close indices.
        self.trees[t].update_ancestors_exits(p, -((e - s) as i64));
        self.trees[t].close(s, e);
        self.reindex(t);
        self.prune_if_isolated(old_parent);
    }

    /// Detach and re-home in one step: the fragment becomes its own
    /// tree, or leaves the forest if it is a lone node.
    fn detach_and_settle(&mut self, child: Id) {
        self.detach(child);

        if self.scratch.order.len() == 1 {
            // No parent and no children means no edges.
            // It's not part of the relation anymore.
            let id = self.scratch.order[0];
            self.locate.remove(id);
            self.scratch.order.clear();
            self.scratch.parent.clear();
            self.scratch.exit.clear();
        } else {
            self.promote_staged();
        }
    }

    /// Set `child`'s parent, returning the previous one.
    ///
    /// Cost is bounded by the trees involved: the subtree is lifted out
    /// of one and spliced into another, and only those two trees are
    /// touched.
    pub(crate) unsafe fn relate(&mut self, child: Id, parent: Id) -> Result<(), RelateError> {
        if child == parent {
            return Err(RelateError::SelfEdgeOnHierarchy(child));
        }

        // Acyclicity is structural rather than searched: relating
        // `child -> parent` closes a loop exactly when `parent` already
        // lies inside `child`'s subtree, and a subtree is a contiguous
        // range. Both must be present for a cycle to be possible.
        if let (Some(&cl), Some(&pl)) = (self.locate.get(child), self.locate.get(parent)) {
            let t = &self.trees[cl.tree];
            if pl.tree == cl.tree && pl.slot >= cl.slot && pl.slot < t.exit[cl.slot] {
                return Err(RelateError::CycleOnAcyclic(child, parent));
            }
        }

        // The edge already exists: the topology is unchanged, so only
        // the value is replaced.
        if self.contains(child, parent) {
            return Ok(());
        }

        // Stage the fragment. A node already in the forest brings its
        // subtree and a new node is alone.
        match self.locate.contains(child) {
            true => self.detach(child),
            false => self.stage_new(child),
        }

        let pl = self.ensure_tree(parent);
        self.attach_staged(pl.tree, pl.slot);
        self.reindex(pl.tree);

        Ok(())
    }

    /// Remove `child`'s edge to `parent`. Its own subtree survives as a
    /// tree of its own; a childless `child` leaves the forest.
    pub(crate) fn unrelate(&mut self, child: Id, parent: Id) {
        if self.contains(child, parent) {
            self.detach_and_settle(child);
        }
    }

    /// Drop every edge touching `id`: it leaves the forest, and each of
    /// its children becomes the root of a tree of its own.
    pub fn purge(&mut self, id: Id) {
        let Some(&l) = self.locate.get(id) else { return };

        // Children are collected as ids. Detaching one
        // shifts its later siblings, so slots captured now would go
        // stale while ids never do.
        let mut kids = std::mem::take(&mut self.scratch.ids);
        kids.clear();
        {
            let tree = &self.trees[l.tree];
            let mut c = tree.first_child(l.slot);
            while let Some(k) = c {
                kids.push(tree.order[k]);
                c = tree.next_sibling(l.slot, k);
            }
        }
        for &k in kids.iter() {
            self.detach_and_settle(k);
        }
        self.scratch.ids = kids;

        // Now a leaf. Detaching evicts it whether it had a parent or
        // was a root, since a lone fragment leaves the forest.
        self.detach_and_settle(id);
        debug_assert!(!self.locate.contains(id));
    }

    /// Remove `id` and every descendant from the forest in one pass.
    ///
    /// The subtree is already a contiguous range, so nothing is staged.
    /// Its payload rows are dropped in place and the hole closes over
    /// them.
    ///
    /// Returns the removed ids in top-down order, appended to `out`, so
    /// the caller can despawn them or run their own cleanup without a
    /// second traversal.
    pub fn purge_subtree(&mut self, id: Id, out: &mut Vec<Id>) {
        let Some(&l) = self.locate.get(id) else { return };
        let (t, s) = (l.tree, l.slot);
        let e = self.trees[t].exit[s];

        let subtree = &self.trees[t].order[s..e];

        out.extend_from_slice(subtree);
        subtree.iter().for_each(|&id| {
            self.locate.remove(id);
        });

        if s == 0 {
            // The whole tree: a root's subtree ends at `len`. `Tree`'s
            // own `Drop` runs the payload dtors for rows 1..len, so
            // discarding it is the entire operation.
            debug_assert_eq!(e, self.trees[t].len());
            self.remove_tree(t);
            return;
        }

        let p = self.trees[t].parent[s];
        let old_parent = self.trees[t].order[p];

        // Ancestors first: `close` renumbers, and `bump_ancestors`
        // walks pre-close indices.
        self.trees[t].update_ancestors_exits(p, -((e - s) as i64));
        // `close`'s precondition is that the hole holds nothing live,
        // which the drop above satisfies just as a move would.
        self.trees[t].close(s, e);
        self.reindex(t);
        self.prune_if_isolated(old_parent);
    }

    #[inline]
    pub fn contains_node(&self, id: Id) -> bool {
        self.locate.contains(id)
    }

    #[inline]
    pub fn parent_of(&self, id: Id) -> Option<Id> {
        self.locate.get(id).and_then(|l| self.trees[l.tree].parent(l.slot))
    }

    /// The stored edge, in the relation's forward direction.
    #[inline]
    pub fn outgoing(&self, id: Id) -> Option<Id> {
        self.parent_of(id)
    }

    /// Direct children. Each child's subtree is contiguous, so stepping
    /// by `exit` reaches the next sibling without visiting descendants.
    #[inline]
    pub fn incoming(&self, id: Id) -> Option<ChildIter<'_>> {
        self.locate.get(id).map(|l| ChildIter {
            tree: &self.trees[l.tree],
            parent: l.slot,
            cur: Some(l.slot + 1).filter(|&c| c < self.trees[l.tree].exit[l.slot]),
        })
    }

    #[inline]
    pub fn has_outgoing(&self, id: Id) -> bool {
        self.locate
            .get(id)
            .is_some_and(|l| self.trees[l.tree].has_parent(l.slot))
    }

    #[inline]
    pub fn has_incoming(&self, id: Id) -> bool {
        self.locate
            .get(id)
            .is_some_and(|l| self.trees[l.tree].has_children(l.slot))
    }

    #[inline]
    pub fn contains(&self, child: Id, parent: Id) -> bool {
        self.parent_of(child).is_some_and(|p| p == parent)
    }

    #[inline]
    pub fn subtree(&self, id: Id) -> &[Id] {
        self.locate.get(id).map_or(&[], |l| self.trees[l.tree].subtree(l.slot))
    }

    /// Transitive containment. Descendants of `a` are exactly the
    /// positions inside its range, so the test is two compares rather
    /// than a walk up `b`'s ancestors.
    #[inline]
    pub fn is_ancestor(&self, a: Id, b: Id) -> bool {
        matches! ((self.locate.get(a), self.locate.get(b)),
            (Some(a), Some(b))
            if a.tree == b.tree && a.slot <= b.slot && b.slot < self.trees[a.tree].exit[a.slot]
        )
    }

    /// Ancestors of `id`, nearest first.
    pub fn ancestors(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        let mut cur = self.locate.get(id).copied();
        std::iter::from_fn(move || {
            let Location { tree: t, slot: p } = cur?;
            let tree = &self.trees[t];
            match tree.parent[p] {
                NONE => {
                    cur = None;
                    None
                }
                q => {
                    cur = Some(Location { tree: t, slot: q });
                    Some(tree.order[q])
                }
            }
        })
    }

    /// Root of `id`'s tree — O(1), where a chain walk would be O(depth).
    #[inline]
    pub fn root_of(&self, id: Id) -> Option<Id> {
        self.locate.get(id).map(|l| self.trees[l.tree].order[0])
    }

    pub fn propagate<I, O>(&mut self, t: u32, input: &[I], output: &mut [O], f: impl FnMut(Option<&O>, &I) -> O) {
        self.trees[t].propagate(input, output, f);
    }

    #[inline]
    pub fn tree_ids(&self, t: u32) -> &[Id] {
        &self.trees[t].order
    }

    #[inline]
    pub fn tree_count(&self) -> u32 {
        self.trees.len()
    }
}

pub struct ChildIter<'a> {
    tree: &'a Tree,
    parent: u32,
    cur: Option<u32>,
}

impl Iterator for ChildIter<'_> {
    type Item = Id;

    #[inline]
    fn next(&mut self) -> Option<Id> {
        self.cur.map(|c| {
            self.cur = self.tree.next_sibling(self.parent, c);
            self.tree.order[c]
        })
    }
}
