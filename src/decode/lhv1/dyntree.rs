//! # Dynamic Huffman Coding.
use std::io;
use core::fmt;
use core::mem::{self, MaybeUninit};
use crate::bitstream::BitRead;
use crate::statictree::entry::*;

#[derive(Clone)]
pub struct DynHuffTree {
    nodes: [TreeNode; NUM_NODES],
    leaves: LeaveIndex,
    groups: Groups,
}

const REORDER_LIMIT: u16 = 32 * 1024;
const NUM_LEAVES: usize = 314;
const NUM_NODES: usize = NUM_LEAVES * 2 - 1;

#[derive(Clone)]
struct Groups {
    ngroups: u16,
    groups: [u16; NUM_NODES], // there will be no more groups than tree nodes
    leaders: [u16; NUM_NODES], // leaders[group] -> node_index
}

#[derive(Clone)]
struct LeaveIndex([u16; NUM_LEAVES]); // leaves[leaf_value] -> node_index

#[derive(Debug, Clone, Copy)]
struct TreeNode {
    /// a leaf or a branch
    entry: TreeEntry,
    /// node frequency
    freq: u16,
    /// parent index
    parent: u16,
    /// frequency group id
    group: u16,
}

impl Default for TreeNode {
    /// Creates an invalid node (a branch pointing to the root) by default.
    fn default() -> TreeNode {
        TreeNode {
            entry: TreeEntry::branch(0).unwrap(),
            freq: 0,
            parent: 0,
            group: 0
        }
    }
}

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
         .field("groups", &&self.groups[..])
         .field("leaders", &&self.leaders[..])
         .finish()
    }
}

impl Groups {
    fn new() -> Self {
        let groups = [0u16; NUM_NODES];
        let mut res = Groups { ngroups: 0, groups, leaders: groups };
        res.reset();
        res
    }

    fn reset(&mut self) {
        self.ngroups = 0;
        for (p, n) in self.groups.iter_mut().zip(0u16..) {
            *p = n;
        }
    }

    #[inline]
    fn allocate(&mut self) -> u16 {
        let ngroups = self.ngroups;
        let res = match self.groups.get(ngroups as usize) {
            Some(&group) => group,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        };
        self.ngroups = ngroups + 1;
        res
    }

    #[inline]
    fn free(&mut self, group: u16) {
        let ngroups = self.ngroups - 1;
        match self.groups.get_mut(ngroups as usize) {
            Some(p) => *p = group,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        };
        self.ngroups = ngroups;
    }

    #[inline]
    fn set_leader_index(&mut self, group: u16, node_index: usize) {
        debug_assert!(node_index < NUM_NODES);
        match self.leaders.get_mut(group as usize) {
            Some(l) => *l = node_index as u16,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        };
    }

    #[inline]
    fn get_leader_index(&self, group: u16) -> usize {
        match self.leaders.get(group as usize) {
            Some(&index) => index as usize,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        }
    }

    #[inline]
    fn set_next_node_as_leader(&mut self, group: u16) {
        match self.leaders.get_mut(group as usize) {
            Some(l) => {
                debug_assert!((*l as usize) < NUM_NODES - 1);
                *l += 1;
            }
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        };
    }
}

impl LeaveIndex {
    #[inline]
    fn set_leaf_node_index(&mut self, value: u16, node_index: usize) {
        debug_assert!(node_index < NUM_NODES);
        match self.0.get_mut(value as usize) {
            Some(l) => *l = node_index as u16,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        };
    }

    #[inline]
    fn get_leaf_node_index(&self, value: u16) -> usize {
        match self.0.get(value as usize) {
            Some(&index) => index as usize,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        }
    }
}

impl TreeNode {
    fn new_leaf(value: u16, group: u16) -> Self {
        debug_assert!((value as usize) < NUM_LEAVES);
        let entry = TreeEntry::leaf(value);
        let freq = 1;
        let parent = 0;
        TreeNode { entry, freq, parent, group }
    }

    fn new_branch(child_index: usize, freq: u16, group: u16) -> Self {
        debug_assert!(child_index < NUM_NODES);
        let entry = TreeEntry::branch(child_index).unwrap();
        let parent = 0;
        TreeNode { entry, freq, parent, group }
    }

    #[inline(always)]
    fn make_branch(&mut self, child_index: usize) {
        debug_assert!(child_index < NUM_NODES);
        self.entry.set_as_branch(child_index);
    }

    #[inline(always)]
    fn is_leaf(&self) -> bool {
        self.entry.is_leaf()
    }

}

impl DynHuffTree {
    pub fn new() -> Self {
        let mut groups = Groups::new();
        let mut nodes = [TreeNode::default(); NUM_NODES];
        let mut leaves = [0u16; NUM_LEAVES];

        let mut last_group = groups.allocate();
        for ((node, leaf), value) in nodes[NUM_NODES - NUM_LEAVES..NUM_NODES]
                           .iter_mut().rev()
                           .zip(leaves.iter_mut())
                           .zip(0..)
        {
            *node = TreeNode::new_leaf(value, last_group);
            *leaf = (NUM_NODES - 1) as u16 - value;
        }

        let mut tail_len = NUM_LEAVES;
        let mut rest = &mut nodes[..];
        let mut last_freq = 0;

        while tail_len > 1 {
            let rest_len = rest.len();
            let parent_len = tail_len / 2;
            let (head, children) = rest.split_at_mut(rest_len - parent_len * 2);
            let head_end = head.len() - (tail_len & 1);
            for ((child_nodes, child_index),
                 (index, parent_node)) in children.rchunks_exact_mut(2)
                                                    .zip((0..rest_len).rev().step_by(2)
                                                  ).zip(head[..head_end].iter_mut()
                                                    .enumerate().rev())
            {
                let mut freq = 0;
                for child in child_nodes.iter_mut() {
                    freq += child.freq;
                    child.parent = index as u16;
                }
                if freq != last_freq {
                    groups.set_leader_index(last_group, index + 1);
                    last_freq = freq;
                    last_group = groups.allocate();
                }
                *parent_node = TreeNode::new_branch(child_index, freq, last_group);
            }
            tail_len -= parent_len;
            rest = head;
        }

        DynHuffTree {
            nodes,
            leaves: LeaveIndex(leaves),
            groups,
        }
    }

    #[inline(never)]
    fn rebuild_tree(&mut self) {
        // move leave entries away maintaining order and dampen down frequency
        let mut leave_nodes: [MaybeUninit<(TreeEntry, u16)>; NUM_LEAVES] = unsafe {
            MaybeUninit::uninit().assume_init()
        };
        // we can't use leaf index, as the current order of leaves should be preserved
        let mut node_filter = self.nodes.iter().filter(|&n| n.is_leaf());
        for t in leave_nodes.iter_mut() {
            let node = node_filter.next().unwrap();
            *t = MaybeUninit::new((node.entry, (node.freq + 1) / 2));
        }
        let leave_nodes = unsafe { //safe, cause all leave nodes has been initialized
            mem::transmute::<_,[(TreeEntry, u16); NUM_LEAVES]>(leave_nodes)
        };
        // an iterator of leaves from last to first
        let mut leaves_riter = leave_nodes.iter().rev();
        // maybe get a next leaf
        let mut next_leaf = leaves_riter.next();
        let mut target_len = NUM_NODES;
        let mut nodes = &mut self.nodes[..];
        while nodes.len() > 2 {
            let child_index = nodes.len() - 1;
            // 1. copy at least 2 more leaves if not enough children
            let num_children = child_index + 1 - target_len;
            if num_children < 2 {
                for node in nodes[..target_len].iter_mut().rev().take(2 - num_children) {
                    let (entry, freq) = next_leaf.unwrap();
                    target_len -= 1;
                    self.leaves.set_leaf_node_index(entry.as_value(), target_len);
                    node.entry = *entry;
                    node.freq = *freq;
                    next_leaf = leaves_riter.next();
                }
            }

            let (head, children) = nodes.split_at_mut(target_len);

            let branch_freq = children.iter().rev().take(2).map(|n| n.freq).sum();
            let mut target_mut = head.iter_mut().rev();

            // 2. copy more leaves until frequency is less
            while let Some((entry, freq)) = next_leaf {
                if branch_freq < *freq {
                    break;
                }
                let node = target_mut.next().unwrap();
                self.leaves.set_leaf_node_index(entry.as_value(), target_mut.len());
                node.entry = *entry;
                node.freq = *freq;
                next_leaf = leaves_riter.next();
            }

            // 3. insert branch
            let mut node = target_mut.next().unwrap();
            node.make_branch(child_index);
            node.freq = branch_freq;
            target_len = target_mut.len();
            for p in children.iter_mut().rev().take(2) {
                p.parent = target_len as u16;
            }

            // 4. repeat until root
            nodes = &mut nodes[..child_index - 1];
        }

        // rebuild groups
        self.groups.reset();
        let mut group = self.groups.allocate();
        let mut freq = self.nodes[0].freq;
        self.nodes[0].group = group;
        self.groups.set_leader_index(group, 0);

        for (node, index) in self.nodes[1..].iter_mut().zip(1..) {
            if node.freq == freq {
                node.group = group;
            } else {
                freq = node.freq;
                group = self.groups.allocate();
                node.group = group;
                self.groups.set_leader_index(group, index);
            }
        }
    }

    #[inline]
    fn set_as_parent(&mut self, child_index: u16, parent_index: usize) {
        let child_index = child_index as usize;
        debug_assert!(parent_index < NUM_NODES);
        #[cfg(debug_assertions)]
        let child_nodes = &mut self.nodes[child_index - 1..child_index + 1];
        #[cfg(not(debug_assertions))]
        let child_nodes = unsafe { self.nodes.get_unchecked_mut(child_index - 1..child_index + 1) };
        for child in child_nodes.iter_mut() {
            child.parent = parent_index as u16;
        }
    }

    #[inline]
    fn promote_to_leader(&mut self, node_index: usize) -> usize {
        let (node, head) = self.nodes[..node_index as usize + 1].split_last_mut().unwrap();
        let leader_index = self.groups.get_leader_index(node.group);

        if leader_index == node_index {
            return node_index
        }
        // swap the new leader with the old one
        let prev_leader = &mut head[leader_index as usize];
        let entry = node.entry;
        node.entry = prev_leader.entry;
        prev_leader.entry = entry;
        // update old leader
        match node.entry.as_type() {
            NodeType::Leaf(value) => {
                self.leaves.set_leaf_node_index(value, node_index);
            }
            NodeType::Branch(child_index) => {
                self.set_as_parent(child_index, node_index);
            }
        }
        // update new leader
        match entry.as_type() {
            NodeType::Leaf(value) => {
                self.leaves.set_leaf_node_index(value, leader_index);
            }
            NodeType::Branch(child_index) => {
                self.set_as_parent(child_index, leader_index);
            }
        }
        leader_index
    }

    #[inline]
    fn increment_frequency(&mut self, node_index: usize) -> &TreeNode {
        let (prev, tail) = self.nodes[node_index - 1..].split_first_mut().unwrap();
        let (node, tail) = tail.split_first_mut().unwrap();

        node.freq += 1;

        // node was part of the group with next nodes
        if let Some(next) = tail.first() {
            if node.group == next.group {
                // the next node is now a leader
                self.groups.set_next_node_as_leader(node.group);
                if node.freq == prev.freq {
                    // join group of previous node
                    node.group = prev.group;
                }
                else {
                    // create node's own group
                    node.group = self.groups.allocate();
                    self.groups.set_leader_index(node.group, node_index);
                }

                return node
            }
        }

        // node had its own group
        if node.freq == prev.freq {
            self.groups.free(node.group);
            // join group of previous node
            node.group = prev.group;
        }
        node
    }

    #[inline]
    fn increment_for_value(&mut self, value: u16) {
        // reorder tree when limit reached
        if self.nodes[0].freq >= REORDER_LIMIT {
            self.rebuild_tree();
        }

        self.nodes[0].freq += 1;

        let mut node_index = self.leaves.get_leaf_node_index(value);
        // walk up from leaf and re-arrange nodes
        while node_index != 0 {
            node_index = self.promote_to_leader(node_index);
            node_index = self.increment_frequency(node_index).parent as usize;
        }
    }

    pub fn read_entry<R: BitRead>(&mut self, mut path: R) -> io::Result<u16> {
        let nodes = &self.nodes;
        let mut node = &nodes[0];
        loop {
            match node.entry.as_type() {
                NodeType::Leaf(value) => {
                    self.increment_for_value(value);
                    return Ok(value)
                }
                NodeType::Branch(index) => {
                    let index = index as usize - path.read_bits::<usize>(1)?;
                    debug_assert!(index < nodes.len());
                    node = unsafe { nodes.get_unchecked(index) };
                    // safe because tree was initialized in a sane way
                }
            }
        }
    }
}

impl fmt::Display for DynHuffTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        fn fmt_step(nodes: &[TreeNode], index: usize, f: &mut fmt::Formatter<'_>, prefix: &mut String) -> fmt::Result {
            let node = nodes[index];
            match node.entry.as_type() {
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

#[cfg(test)]
mod tests {
    use rand::{Rng, RngCore, thread_rng};
    use rand::distributions::{Uniform, WeightedIndex};
    use crate::bitstream::BitStream;
    use std::collections::{HashSet, HashMap};
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
            match node.entry.as_type() {
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
        for (&value, &index) in leaves.iter() {
            assert_eq!(tree.leaves.get_leaf_node_index(value), index);
        }
        // all leaves should be reachable and on the unique path
        fn into_branch(nodes: &[TreeNode], index: usize, leaves: &mut HashSet<u16>) {
            match nodes[index].entry.as_type() {
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
        println!("{}", tree);
        for i in 0..NUM_LEAVES {
            for _ in 0..i {
                tree.increment_for_value(i as u16);
            }
        }
        validate_tree(&tree);
        println!("--------------\n{}", tree);

        let mut trng = thread_rng();
        let rng = &mut trng;
        // spam tree with random values
        let mut tree = DynHuffTree::new();
        for sample in rng.sample_iter(Uniform::new(0, NUM_LEAVES)).take(1_000_000) {
            tree.increment_for_value(sample as u16);
        }
        validate_tree(&tree);
        println!("--------------\n{}", tree);

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
        println!("--------------\n{}", tree);

        // now with some random bit stream
        let rnd_stream: &mut (dyn RngCore) = rng;
        let mut rndstream = BitStream::new(rnd_stream);
        for _ in 0..1_000_000 {
            tree.read_entry(&mut rndstream).unwrap();
        }
        validate_tree(&tree);
        println!("--------------\n{}", tree);
    }
}
