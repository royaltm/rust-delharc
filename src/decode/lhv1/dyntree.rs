//! # Dynamic Huffman Coding.
#[cfg(all(test, not(feature = "std")))]
use alloc::string::String;
#[cfg(not(feature = "std"))]
use alloc::boxed::Box;
use core::{fmt, mem};
use bytemuck::{AnyBitPattern, NoUninit, Zeroable, cast_slice_mut, allocation::try_zeroed_box};
use crate::{
    error::LhaError,
    bitstream::BitRead,
    statictree::{NodeType, TreeEntry}
};

/// Dynamic Huffman Tree
#[derive(Clone, Zeroable)]
pub struct DynHuffTree {
    /// The tree
    nodes: [TreeNode; NUM_NODES],
    /// An offset to a given leaf indexed by its value
    leaves: LeavesIndex,
    /// Frequency groups and group leaders
    groups: Groups,
}

/// If a root node frequency is equal or exceed this value a whole tree is rebuilt
const REORDER_LIMIT: u16 = 32 * 1024;
/// The number of leaves, or unique values, stored in a dynamic tree.
const NUM_LEAVES: usize = 314;
/// The total number of tree nodes - leaves and branches.
const NUM_NODES: usize = NUM_LEAVES * 2 - 1;

/// An object holding a leaf and its frequency.
///
/// This object is used when the dynamic tree is rebuilt
#[derive(Clone, Copy, NoUninit, AnyBitPattern)]
#[repr(C)]
struct LeafNode {
    entry: TreeEntry,
    freq: u16
}

/// A tuple of properties for an interleaved array of group identifiers
/// and leaders
#[derive(Debug, Clone, Copy, NoUninit, AnyBitPattern)]
#[repr(C)]
struct GroupOrLeader {
    /// A unique group identifier
    group: u16,
    /// An index to a tree node group leader
    leader: u16
}

/// This object is being used for allocating new frequency groups and
/// finding group leaders
#[derive(Clone, Copy, Zeroable)]
#[repr(C)]
struct Groups {
    /// The number of allocated groups.
    ///
    /// There will never be more groups than tree nodes.
    ngroups: u16,
    /// An array of unique group identifiers and leader node indexes.
    ///
    /// To access a group identifier, this array is indexed by [`Self::ngroups`].
    ///
    /// To access a group leader, this array is indexed by group identifiers.
    groups_leaders: [GroupOrLeader; NUM_NODES], // groups_leaders[group].leader -> node_index
}

/// An index object for looking up leaves in the tree by their values
#[derive(Clone, Copy, Zeroable)]
#[repr(transparent)]
struct LeavesIndex([u16; NUM_LEAVES]); // leaves[leaf_value] -> node_index

/// A single tree object node with additional attributes
#[derive(Debug, Clone, Copy, Zeroable)]
#[repr(C)]
struct TreeNode {
    /// A leaf or a branch containing a value or an index to children of a branch 
    entry: TreeEntry,
    /// A frequency of tree node.
    ///
    /// The tree must be sorted by descending node frequencies at all times.
    ///
    /// * A leaf frequency is determined by how many times it was read from the tree.
    /// * A branch frequency is a sum of frequencies of all descendant nodes.
    ///
    freq: u16,
    /// An index to a parent node
    parent: u16,
    /// A frequency group identifier.
    ///
    /// Tree nodes having the same frequency must belong to a single and unique group.
    group: u16,
}

// Unsafe asserts are used to guide the optimizer
macro_rules! unsafe_assert {
    ($expr:expr) => {
        #[cfg(all(not(feature = "no-unsafe-assertions"), not(debug_assertions)))]
        unsafe {
            core::hint::assert_unchecked($expr)
        }
        debug_assert!($expr)
    };
}

/// This macro asserts that a tree leaf value is contained in an allowed range
macro_rules! unsafe_assert_leaf_value_in_range {
    ($value:ident) => {
        unsafe_assert!($value < const { NUM_LEAVES as u16 })
    };
}

/// This macro asserts that a tree branch child index is contained in an allowed range
macro_rules! unsafe_assert_child_index_in_range {
    ($child_index:ident) => {
        unsafe_assert!(usize::from($child_index) > 0 && usize::from($child_index) < NUM_NODES)
    };
}

/// This macro asserts that a group identifier is in an allowed range
macro_rules! unsafe_assert_group_in_range {
    ($group:ident) => {
        unsafe_assert!($group < const { NUM_NODES as u16 })
    };
}

/// This macro asserts that there can be no more groups than nodes
macro_rules! unsafe_assert_group_can_allocate {
    ($groups:expr) => {
        unsafe_assert!($groups.ngroups < const { NUM_NODES as u16 })
    };
}

/// This macro asserts that a group can be freed
macro_rules! unsafe_assert_group_can_free {
    ($groups:expr) => {
        unsafe_assert!($groups.ngroups > 0 && $groups.ngroups <= const { NUM_NODES as u16 })
    };
}

// impl Default for TreeNode {
//     /// Creates an invalid node (a branch pointing to the root) by default.
//     fn default() -> TreeNode {
//         TreeNode {
//             entry: TreeEntry::branch(0),
//             freq: 0,
//             parent: 0,
//             group: 0
//         }
//     }
// }

impl fmt::Debug for DynHuffTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynHuffTree")
         .field("nodes", &&self.nodes[..])
         .field("leaves", &&self.leaves.0[..])
         .field("groups", &self.groups)
         .finish()
    }
}

impl fmt::Debug for Groups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Groups")
         .field("ngroups", &self.ngroups)
         .field("groups_leaders", &&self.groups_leaders[..])
         .finish()
    }
}

impl Groups {
    // #[inline]
    // fn new() -> Self {
    //     Groups {
    //         ngroups: 0,
    //         groups_leaders: core::array::from_fn(|group|
    //             GroupOrLeader { group: group as u16, leader: 0 })
    //     }
    // }

    /// Assign unique group identifiers and reset the allocation index
    #[inline]
    fn reset(&mut self) {
        self.ngroups = 0;
        for (gl, n) in self.groups_leaders.iter_mut().zip(0u16..) {
            gl.group = n;
        }
    }
    /// Allocate a single group identifier.
    ///
    /// # Note
    /// The caller must ensure there will be no more allocations than tree nodes.
    #[inline]
    fn allocate(&mut self) -> u16 {
        let ngroups = self.ngroups;
        let res = self.groups_leaders[usize::from(ngroups)].group;
        self.ngroups = ngroups + 1;
        res
    }
    /// Free a single group identifier.
    ///
    /// # Note
    /// The caller must ensure there are no more tree nodes using this group and
    /// the identifier was previously allocated using [`Self::allocate`].
    #[inline]
    fn free(&mut self, group: u16) {
        debug_assert!(group < NUM_NODES as u16);
        let ngroups = self.ngroups - 1;
        self.groups_leaders[usize::from(ngroups)].group = group;
        self.ngroups = ngroups;
    }
    /// Assign a leader node index to a group identifier.
    ///
    /// # Note
    /// The caller must ensure the group identifier was previously allocated
    /// using [`Self::allocate`] and the node index must point to a tree node.
    #[inline]
    fn set_leader_index(&mut self, group: u16, node_index: usize) {
        debug_assert!(node_index < NUM_NODES);
        self.groups_leaders[usize::from(group)].leader = node_index as u16;
    }
    /// Acquire an index to a leader node of a group identifier.
    ///
    /// # Note
    /// The caller must ensure the group identifier was previously allocated
    /// using [`Self::allocate`] and that the group has a leader previously
    /// assigned.
    #[inline]
    fn get_leader_index(&self, group: u16) -> usize {
        usize::from(self.groups_leaders[usize::from(group)].leader)
    }
    /// Bump a leader node of a group identifier to the next node.
    ///
    /// # Note
    /// The caller must ensure the group identifier was previously allocated
    /// using [`Self::allocate`], the group has a previously assigned leader
    /// and that the previous leader is not the last node in the tree.
    #[inline]
    fn set_next_node_as_leader(&mut self, group: u16) {
        let gl = &mut self.groups_leaders[usize::from(group)];
        debug_assert!((usize::from(gl.leader)) < NUM_NODES - 1);
        gl.leader += 1;
    }
}

impl LeavesIndex {
    /// Set an index to a leaf node in the tree by the value of a leaf.
    ///
    /// # Note
    /// The caller must ensure the value is within the allowed range and
    /// that the node index points to a valid leaf node in the tree.
    #[inline]
    fn set_leaf_node_index(&mut self, value: u16, node_index: usize) {
        debug_assert!(node_index < NUM_NODES);
        self.0[usize::from(value)] = node_index as u16;
    }
    /// Get an index to a leaf node in the tree by the value of a leaf.
    ///
    /// # Note
    /// The caller must ensure the value is within the allowed range and that
    /// the node index was previously set using [`Self::set_leaf_node_index`].
    #[inline]
    fn get_leaf_node_index(&self, value: u16) -> usize {
        usize::from(self.0[usize::from(value)])
    }
}

impl TreeNode {
    /// Create a new tree leaf with default attributes.
    ///
    /// # Note
    /// The caller must ensure the value is within the allowed range and that
    /// the group identifier was previously allocated. Each tree leaf must have
    /// a unique value assigned. The leaf also needs a proper parent assigned
    /// once it is placed in the tree.
    #[inline]
    fn new_leaf(value: u16, group: u16) -> Self {
        debug_assert!(usize::from(value) < NUM_LEAVES);
        debug_assert!(usize::from(group) < NUM_NODES);
        let entry = TreeEntry::leaf(value);
        let freq = 1;
        let parent = 0;
        TreeNode { entry, freq, parent, group }
    }
    /// Create a new tree branch with default attributes.
    ///
    /// # Note
    /// The caller must ensure the child index is within the allowed range and
    /// that the group identifier was previously allocated. Each tree branch
    /// must point to a unique pair of child nodes, with the child index pointing
    /// at the second child node. The branch also needs a proper parent assigned
    /// once it is placed in the tree, unless it is a root branch.
    #[inline]
    fn new_branch(child_index: usize, freq: u16, group: u16) -> Self {
        debug_assert!(child_index < NUM_NODES);
        debug_assert!(usize::from(group) < NUM_NODES);
        debug_assert!((2..=NUM_LEAVES).contains(&usize::from(freq)));
        let entry = TreeEntry::branch(child_index);
        let parent = 0;
        TreeNode { entry, freq, parent, group }
    }
    /// Replace a tree node with a branch node.
    ///
    /// The attributes of a node remain unchanged.
    ///
    /// # Note
    /// The caller must ensure the child index is within the allowed range.
    /// Each branch must point to a unique pair of child nodes, with the
    /// child index pointing at the second child node.
    #[inline]
    fn make_branch(&mut self, child_index: usize) {
        debug_assert!(child_index < NUM_NODES);
        self.entry.set_as_branch(child_index);
    }
    /// Return whether a tree node is a leaf
    #[inline]
    fn is_leaf(&self) -> bool {
        self.entry.is_leaf()
    }

}

impl DynHuffTree {
    /// Create a new boxed [`DynHuffTree`], ready to read entries from.
    pub fn new() -> Box<Self> {
        // Allocate an invalid tree directly on the heap to avoid large stack allocation.
        let mut tree = try_zeroed_box::<DynHuffTree>().expect("not enough memory for a dynamic tree");
        let groups = &mut tree.groups;
        let nodes = &mut tree.nodes;

        // Initialize leaves index:
        // All leaves will be placed at the bottom of the tree in their value-reverse order.
        for (leaves_index, value) in tree.leaves.0.iter_mut().zip(0u16..) {
            *leaves_index = const { NUM_NODES as u16 - 1 } - value;
        }

        // Initialize group identifiers and reset group allocations:
        groups.reset();
        // SAFETY: groups have been reset
        unsafe_assert_group_can_allocate!(groups);
        let mut last_group = groups.allocate();
        // Initialize leaves:
        // All leaves are placed at the bottom of the tree in their value-reverse order.
        for (node, value) in nodes[NUM_NODES - NUM_LEAVES..NUM_NODES]
                             .iter_mut().rev()
                             .zip(0..)
        {
            *node = TreeNode::new_leaf(value, last_group);
        }

        // Initialize branches:
        let mut last_freq = 0;
        // Branches are initialized back to front
        for child_index in (2..NUM_NODES).rev().step_by(2) {
            let index = child_index / 2 - 1;
            // the rust optimizer can see that child_index is in 2..NUM_NODES
            // and thus also index < NUM_NODES / 2 - 1
            let child_nodes = &mut nodes[child_index - 1..=child_index];
            let mut freq = 0;
            for child in child_nodes.iter_mut() {
                freq += child.freq;
                child.parent = index as u16;
            }
            if freq != last_freq {
                // SAFETY: the group has been allocated and index < NUM_NODES / 2 - 1
                unsafe_assert_group_in_range!(last_group);
                // assign an index to a next position node
                groups.set_leader_index(last_group, index + 1);
                last_freq = freq;
                // SAFETY: this will be called less times than NUM_NODES
                // by the virtue of the loop constraint
                unsafe_assert_group_can_allocate!(groups);
                last_group = groups.allocate();
            }
            nodes[index] = TreeNode::new_branch(child_index, freq, last_group);
        }
        tree
    }

    #[inline(never)]
    fn rebuild_tree(&mut self) {
        // Use groups_leaders slice as a temporary leaves storage,
        // groups along with leaders are fully recreated at the end of this function
        assert_eq!(size_of::<LeafNode>(), size_of::<GroupOrLeader>());
        let leaf_nodes: &mut [LeafNode] = cast_slice_mut(&mut self.groups.groups_leaders[..NUM_LEAVES]);
        debug_assert_eq!(leaf_nodes.len(), NUM_LEAVES);
        // Move leaf entries away, maintaining order and dampen down frequency
        // we can't use leaf index, as the current order of leaves should be preserved.
        // Copy leaves back to front, so their position is reversed in leaf_nodes.
        let mut node_filter = self.nodes.iter().rev().filter(|&n| n.is_leaf());
        for leaf in leaf_nodes.iter_mut() {
            let node = node_filter.next().unwrap(); // there shall be NUM_LEAVES leaves
            *leaf = LeafNode { entry: node.entry, freq: node.freq.div_ceil(2) };
        }
        debug_assert!(node_filter.next().is_none());
        // A leaf iterator from the last one to the first
        let mut leaves_riter = leaf_nodes.iter();
        // Rebuilding nodes:
        let mut target_index = NUM_NODES - 1; // last target slot
        let mut child_index = NUM_NODES - 1; // last child slot
        let nodes = &mut self.nodes;
        let mut branch_freq = 0u16; // 0 = no frequency calculated
        'leaves: loop {
            let next_leaf = leaves_riter.next();
            loop {
                if target_index >= NUM_NODES {
                    // this ending condition also optimizes out slice boundary check
                    break 'leaves
                }
                // SAFETY: child_index starts at NUM_NODES - 1
                //         child_index is decreased by 2 only after
                //         asserting that child_index >= target_index + 2
                //         thus child_index can never overflow
                // this hint together with an assert helps eliminate slice boundary checks
                unsafe_assert!(child_index < NUM_NODES);
                let node = &mut nodes[target_index];
                if let Some(leaf) = next_leaf &&
                   (leaf.freq <= branch_freq || (child_index - target_index) < 2)
                {
                    // 1. copy leaves to have at least 2 outstanding or if leaves have <= frequency
                    let value = leaf.entry.as_value();
                    // SAFETY: leaf value must be valid
                    unsafe_assert_leaf_value_in_range!(value);
                    // ensure the leaf node index is updated
                    self.leaves.set_leaf_node_index(value, target_index);
                    node.entry = leaf.entry;
                    node.freq = leaf.freq;
                    target_index -= 1; // next target, this shall never overflow under normal conditions
                    continue 'leaves
                }
                else {
                    // Ensure validity of leaves, this also prevents child_index from overflowing on sub
                    assert!(child_index >= target_index + 2);
                    if branch_freq == 0 {
                        // 2. calculate branch frequency from last 2 children and maybe copy more leaves
                        branch_freq = nodes[child_index - 1..=child_index].iter().map(|n| n.freq).sum();
                    }
                    else {
                        // 3. insert branch
                        node.make_branch(child_index);
                        node.freq = branch_freq;
                        branch_freq = 0; // next branch
                        for n in nodes[child_index - 1..=child_index].iter_mut() {
                            n.parent = target_index as u16; // link with a parent
                        }
                        // this shall not overflow, see assertion above 
                        child_index -= 2; // next 2 children
                        target_index = target_index.wrapping_sub(1); // next target or end on overflow
                    }
                }
            }
        }
        debug_assert_eq!(leaves_riter.len(), 0);

        // Rebuild groups and their leaders
        let groups = &mut self.groups;
        groups.reset();
        let mut freq = nodes[0].freq;
        // SAFETY: groups have been reset
        unsafe_assert_group_can_allocate!(groups);
        let mut group = groups.allocate();
        // SAFETY: A unique group has been allocated
        unsafe_assert_group_in_range!(group);
        nodes[0].group = group;
        // The root node always belong to its own group because
        // its frequency is a sum of all tree node frequencies.
        groups.set_leader_index(group, 0);
        // Tree nodes are all sorted by frequency at this point
        // Scan nodes and assign unique groups and their leaders
        for (node, index) in nodes[1..].iter_mut().zip(1..) {
            if node.freq == freq {
                node.group = group;
            }
            else {
                freq = node.freq;
                // SAFETY: the loop is iterated the number of times
                // exactly matching the number of potential groups
                unsafe_assert_group_can_allocate!(groups);
                group = groups.allocate();
                // SAFETY: A unique group has been allocated
                unsafe_assert_group_in_range!(group);
                node.group = group;
                groups.set_leader_index(group, index);
            }
        }
    }
    /// Set a parent of a pair of child nodes.
    ///
    /// The child_index must point to a second child.
    #[inline]
    fn set_as_parent(&mut self, child_index: u16, parent_index: usize) {
        debug_assert!(parent_index < NUM_NODES);
        let child_index = usize::from(child_index);
        let child_nodes = &mut self.nodes[child_index - 1..=child_index];
        for child in child_nodes.iter_mut() {
            child.parent = parent_index as u16;
        }
    }
    /// Ensure a node is a leader or promote a node to the leader position
    /// return the new valid node position.
    #[inline]
    fn promote_to_leader(&mut self, node_index: usize) -> usize {
        let (node, head) = self.nodes[..=node_index].split_last_mut().unwrap();
        let leader_index = {
            let group = node.group;
            // SAFETY: A node group is allocated before it is being assigned to a node.
            unsafe_assert_group_in_range!(group);
            self.groups.get_leader_index(group)
        };
        assert!(head.len() == node_index); // trivial to prove compile-time
        // if a node_index were a leader then the following condition fails:
        let leader = if leader_index < head.len() {
            &mut head[leader_index] // no boundary check here
        }
        else {
            assert!(leader_index == node_index); // group leader can only be <= node_index
            return node_index
        };
        // swap the new leader with the old one
        let prev_entry = leader.entry;
        let node_entry = mem::replace(&mut node.entry, prev_entry);
        leader.entry = node_entry;
        // update old leader
        match prev_entry.as_node() {
            NodeType::Leaf(value) => {
                // SAFETY: leaf value must be valid
                unsafe_assert_leaf_value_in_range!(value);
                // ensure the leaf node index is updated
                self.leaves.set_leaf_node_index(value, node_index);
            }
            NodeType::Branch(child_index) => {
                // SAFETY: branch child_index must be valid
                unsafe_assert_child_index_in_range!(child_index);
                // ensure the leaf children are updated
                self.set_as_parent(child_index, node_index);
            }
        }
        // update new leader
        match node_entry.as_node() {
            NodeType::Leaf(value) => {
                // SAFETY: leaf value must be valid
                unsafe_assert_leaf_value_in_range!(value);
                // ensure the leaf node index is updated
                self.leaves.set_leaf_node_index(value, leader_index);
            }
            NodeType::Branch(child_index) => {
                // SAFETY: branch child_index must be valid
                unsafe_assert_child_index_in_range!(child_index);
                // ensure the leaf children are updated
                self.set_as_parent(child_index, leader_index);
            }
        }
        leader_index
    }
    /// Increment the frequency of a node and return a reference to
    /// a tree node object.
    ///
    /// The index must not point to a root node because this function inspects the
    /// previous node position.
    ///
    /// The caller must ensure the given node index must point to a frequency group leader
    /// and the call to this function must be repeated for all ancestors, ensuring each
    /// ancestor is also a group leader.
    #[inline]
    fn increment_frequency(&mut self, node_index: usize) -> &TreeNode {
        let (prev, node_tail) = self.nodes[node_index - 1..].split_first_mut().unwrap();
        let (node, tail) = node_tail.split_first_mut().unwrap();

        // Bump frequency.
        // To prevent overflowing, a caller must ensure a tree is rebuilt
        // when root node frequency is at a certain threshold.
        node.freq += 1;

        let groups = &mut self.groups;

        // Check if a node was part of the group with more members
        if let Some(next) = tail.first() && node.group == next.group {
            // The next node is now a leader of its group
            let group = node.group;
            // SAFETY: A node group is allocated before it is being assigned to a node.
            unsafe_assert_group_in_range!(group);
            // We made sure there is a next node we can bump the leader position to
            groups.set_next_node_as_leader(group);
            // Check if a bumped node frequency matches the previous node frequency
            if node.freq == prev.freq {
                // Simply join the group of a previous node
                node.group = prev.group;
            }
            else {
                // Create a unique group for a single node
                // SAFETY: we know there are more nodes belonging to the same group
                // at this point, so there must be at least one unique group available
                unsafe_assert_group_can_allocate!(groups);
                let group = groups.allocate();
                // SAFETY: A unique group has been allocated
                unsafe_assert_group_in_range!(group);
                node.group = group;
                // Set this node as a leader of its own group
                groups.set_leader_index(group, node_index);
            }
        }
        // The node was a single member of a unique group
        else if node.freq == prev.freq {
            // A bumped node frequency matches the previous node frequency
            // SAFETY: A node group is allocated before it is being assigned to a node
            // and we determined there are no more members of this group
            unsafe_assert_group_can_free!(groups);
            groups.free(node.group);
            // Join the group of a previous node
            node.group = prev.group;
        }
        // Otherwise a node remains a single member of a unique group
        node
    }
    /// Increment frequency of a leaf node and all of its ancestors and rebuild
    /// the tree if necessary.
    ///
    /// The leaf is identified by the unique value it represents.
    #[inline]
    fn increment_for_value(&mut self, value: u16) {
        // Dumpen frequencies and reorder the tree when a limit is reached
        if self.nodes[0].freq >= REORDER_LIMIT {
            self.rebuild_tree();
        }
        // Bump the frequency of a tree root
        self.nodes[0].freq += 1;
        // Look up the leaf node
        let mut node_index = self.leaves.get_leaf_node_index(value);
        // Walk up from leaf through all ancestors and re-arrange nodes
        while node_index != 0 { // loop until root is found
            // SAFETY: leaf lookup index is never populated with indexes outside
            // of the allowed range
            unsafe_assert_child_index_in_range!(node_index);
            node_index = self.promote_to_leader(node_index);
            // SAFETY: leader lookup index is never populated with indexes outside
            // of the allowed range and we never point to the root node
            unsafe_assert_child_index_in_range!(node_index);
            node_index = usize::from(self.increment_frequency(node_index).parent);
        }
    }

    /// Read an entry value from the dynamic tree.
    ///
    /// The returned entry values are in the range: `0..314`.
    pub fn read_entry<R: BitRead>(&mut self, mut path: R) -> Result<u16, LhaError<R::Error>> {
        let nodes = &self.nodes;
        let mut node = &nodes[0];
        loop {
            match node.entry.as_node() {
                NodeType::Leaf(value) => {
                    // SAFETY: leaf value must be valid
                    unsafe_assert_leaf_value_in_range!(value);
                    self.increment_for_value(value);
                    return Ok(value)
                }
                NodeType::Branch(child_index) => {
                    let is_one = path.read_bit()?;
                    let child_index = usize::from(child_index);
                    // SAFETY: branch child_index must be valid
                    unsafe_assert_child_index_in_range!(child_index);
                    node = if is_one {
                        &nodes[child_index - 1]
                    }
                    else {
                        &nodes[child_index]
                    };
                }
            }
        }
    }
}

#[cfg(test)]
impl fmt::Display for DynHuffTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        fn fmt_step(nodes: &[TreeNode], index: usize, f: &mut fmt::Formatter<'_>, prefix: &mut String) -> fmt::Result {
            let node = nodes[index];
            match node.entry.as_node() {
                NodeType::Leaf(code) => writeln!(f, "{} -> {} f: {} g: {}", prefix, code, node.freq, node.group)?,
                NodeType::Branch(index) => {
                    prefix.push('0');
                    fmt_step(nodes, index as usize, f, prefix)?;
                    prefix.pop();
                    prefix.push('1');
                    fmt_step(nodes, index as usize - 1, f, prefix)?;
                    prefix.pop();
                }
            }
            Ok(())
        }

        let mut prefix = String::new();
        fmt_step(&self.nodes, 0, f, &mut prefix)
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use std::collections::{HashSet, HashMap};
    use rand::{RngExt, RngReader};
    use rand::distr::{Uniform, weighted::WeightedIndex};
    use crate::bitstream::BitStream;
    use super::*;

    fn validate_tree(tree: &DynHuffTree) {
        let mut leaves: HashMap<u16, usize> = HashMap::with_capacity(NUM_LEAVES);
        let mut children: HashSet<u16> = HashSet::with_capacity(NUM_NODES);
        let mut groups: HashSet<u16> = HashSet::with_capacity(NUM_NODES);
        let mut freq = u16::max_value();
        let mut group = u16::max_value();
        // root is a branch
        assert!(!tree.nodes[0].is_leaf());
        assert_eq!(tree.nodes[0].parent, 0);
        for (index, node) in tree.nodes.iter().enumerate() {
            match node.entry.as_node() {
                NodeType::Leaf(value) => {
                    // all leaves should be unique
                    assert!((value as usize) < NUM_LEAVES);
                    assert!(leaves.insert(value, index).is_none());
                }
                NodeType::Branch(child_index) => {
                    // invalid (default) node should not be present
                    assert!(child_index != 0);
                    // all child indexes should be even
                    assert!(child_index & 1 == 0);
                    // there must be no duplicate parents of the same children
                    assert!(children.insert(child_index));
                    // parent index should match
                    let child0 = &tree.nodes[child_index as usize - 1];
                    let child1 = &tree.nodes[child_index as usize];
                    assert_eq!(child0.parent as usize, index);
                    assert_eq!(child1.parent as usize, index);
                    // freq should be a sum of children's frequency
                    assert_eq!(child0.freq + child1.freq, node.freq);
                }
            }
            // check frequencies are descending and groups are consistent
            assert!(node.freq <= freq);
            if node.freq == freq {
                assert_eq!(node.group, group);
            }
            else {
                assert_ne!(node.group, group);
            }
            if node.group == group {
                assert_eq!(node.freq, freq);
            }
            else {
                assert_ne!(node.freq, freq);
                assert!((node.group as usize) < NUM_NODES);
                // groups should be unique
                assert!(groups.insert(node.group));
                // this must be a group leader
                assert_eq!(tree.groups.get_leader_index(node.group), index);
                group = node.group;
                freq = node.freq;
            }
            // parents should all meet at root
            let mut parent_index = node.parent as usize;
            while parent_index != 0 {
                let parent_node = tree.nodes[parent_index];
                assert!(!parent_node.is_leaf());
                assert!(parent_node.freq > node.freq);
                assert_ne!(parent_node.group, node.group);
                parent_index = parent_node.parent as usize;
            }
        }
        // all leaves should be present
        assert_eq!(leaves.len(), NUM_LEAVES);
        // validate leaves index
        for (&value, &index) in leaves.iter() {
            assert_eq!(tree.leaves.get_leaf_node_index(value), index);
        }
        // all leaves should be reachable and on the unique path
        fn into_branch(nodes: &[TreeNode], index: usize, leaves: &mut HashSet<u16>) {
            match nodes[index].entry.as_node() {
                NodeType::Leaf(code) => {
                    assert!(leaves.insert(code));
                }
                NodeType::Branch(index) => {
                    into_branch(nodes, index as usize - 1, leaves);
                    into_branch(nodes, index as usize, leaves);
                }
            }
        }
        let mut leaves: HashSet<u16> = HashSet::with_capacity(NUM_LEAVES);
        into_branch(&tree.nodes, 0, &mut leaves);
        assert_eq!(leaves.len(), NUM_LEAVES);
    }

    #[test]
    fn dyntree_works() {
        let mut tree = DynHuffTree::new();
        validate_tree(&tree);
        println!("{:?}\n{}", tree, tree);
        for i in 0..NUM_LEAVES {
            for _ in 0..i {
                tree.increment_for_value(i as u16);
            }
        }
        validate_tree(&tree);
        println!("-------------- [1]\n{}", tree);

        let mut trng = rand::rng();

        // now with some random bit stream
        let mut rndstream = BitStream::new(RngReader(&mut trng));
        for _ in 0..1_000_000 {
            assert!(usize::from(tree.read_entry(&mut rndstream).unwrap()) < NUM_LEAVES);
        }
        validate_tree(&tree);
        println!("-------------- [2]\n{}", tree);

        let rng = &mut trng;

        // spam tree with random values
        let mut tree = DynHuffTree::new();
        for sample in rng.sample_iter(Uniform::new(0, NUM_LEAVES).unwrap()).take(1_000_000) {
            tree.increment_for_value(sample as u16);
        }
        validate_tree(&tree);
        println!("-------------- [3]\n{}", tree);

        // spam tree with some random, and non-uniformly distributed values
        let mut weights = [0u16;NUM_LEAVES];
        rng.fill(&mut weights[..]);
        let dist = WeightedIndex::new(
            weights.iter().map(|&n| (n as u64)*(n as u64)) // boost weights
        ).unwrap();
        for sample in rng.sample_iter(dist).take(1_000_000) {
            tree.increment_for_value(sample as u16);
        }
        validate_tree(&tree);
        println!("-------------- [4]\n{}", tree);

        // now with some random bit stream
        let mut rndstream = BitStream::new(RngReader(rng));
        for _ in 0..1_000_000 {
            assert!(usize::from(tree.read_entry(&mut rndstream).unwrap()) < NUM_LEAVES);
        }
        validate_tree(&tree);
        println!("-------------- [5]\n{}", tree);
    }
}
