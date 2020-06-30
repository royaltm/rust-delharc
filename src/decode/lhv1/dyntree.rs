//! # Dynamic Huffman Coding.
use std::io;
use core::fmt;
use core::mem::{self, MaybeUninit};
use modular_bitfield::prelude::*;
use crate::bitstream::BitRead;

#[derive(Clone)]
pub struct DynHuffTree {
    nodes: [TreeNode; NUM_NODES],
    leaves: [u16; NUM_LEAVES], // leaves[leaf_value] -> node_index
    leaders: [u16; NUM_NODES], // leaders[group] -> node_index
    groups: Groups,
}

const REORDER_LIMIT: u16 = 32 * 1024;
const NUM_LEAVES: usize = 314;
const NUM_NODES: usize = NUM_LEAVES * 2 - 1;

#[derive(Clone, Copy)]
enum NodeType {
    Leaf(u16),
    Branch(u16),
}

#[bitfield]
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
struct IndexLeaf {
    leaf: bool,         // Uses 1 bit
    value: B15          // Uses 15 bits
}

#[derive(Clone)]
struct Groups {
    groups: [u16; NUM_NODES], // there will be no more groups than tree nodes
    ngroups: u16
}

#[derive(Debug, Default, Clone, Copy)]
struct TreeNode {
    // leaf value or child index
    value: IndexLeaf,
    // node frequency
    freq: u16,
    // parent index
    parent: u16,
    // frequency group id
    group: u16,
}

impl fmt::Debug for DynHuffTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynHuffTree")
         .field("nodes", &&self.nodes[..])
         .field("leaves", &&self.leaves[..])
         .field("leaders", &&self.leaders[..])
         .field("groups", &self.groups)
         .finish()
    }
}

impl fmt::Debug for Groups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Groups")
         .field("groups", &&self.groups[..])
         .field("ngroups", &self.ngroups)
         .finish()
    }
}

impl Groups {
    fn new() -> Self {
        let groups = [0u16; NUM_NODES];
        let mut res = Groups { groups, ngroups: 0 };
        res.reset();
        res
    }

    fn reset(&mut self) {
        self.ngroups = 0;
        for (p, n) in self.groups.iter_mut().zip(0u16..) {
            *p = n;
        }
    }

    fn allocate(&mut self) -> u16 {
        let ngroups = self.ngroups;
        let res = match self.groups.get(ngroups as usize) {
            Some(group) => group,
            #[cfg(debug_assertions)]
            None => unreachable!(),
            #[cfg(not(debug_assertions))]
            None => unsafe { core::hint::unreachable_unchecked() }
        };
        self.ngroups = ngroups + 1;
        *res
    }

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
}

impl IndexLeaf {
    fn leaf(value: u16) -> Self {
        debug_assert!((value as usize) < NUM_LEAVES);
        let mut index = IndexLeaf::new();
        index.set_leaf(true);
        index.set_value(value);
        index
    }

    fn branch(child_index: u16) -> Self {
        debug_assert!((child_index as usize) < NUM_NODES);
        let mut index = IndexLeaf::new();
        index.set_value(child_index);
        index
    }

    #[inline(always)]
    fn as_type(&self) -> NodeType {
        if self.get_leaf() {
            NodeType::Leaf(self.get_value())
        }
        else {
            NodeType::Branch(self.get_value())
        }
    }
}

impl TreeNode {
    fn new_leaf(value: u16, group: u16) -> Self {
        let value = IndexLeaf::leaf(value);
        let freq = 1;
        let parent = 0;
        TreeNode { value, freq, parent, group }
    }

    fn new_branch(child_index: u16, freq: u16, group: u16) -> Self {
        let value = IndexLeaf::branch(child_index);
        let parent = 0;
        TreeNode { value, freq, parent, group }
    }

    #[inline(always)]
    fn make_branch(&mut self, index: u16) {
        self.value.set_leaf(false);
        self.value.set_value(index);
    }

    #[inline(always)]
    fn is_leaf(&self) -> bool {
        self.value.get_leaf()
    }

}

impl DynHuffTree {
    pub fn new() -> Self {
        let mut groups = Groups::new();
        let mut nodes = [TreeNode::default(); NUM_NODES];
        let mut leaves = [0u16; NUM_LEAVES];
        let mut leaders = [0u16; NUM_NODES];

        let mut last_group = groups.allocate();
        for (val, (node, leaf)) in nodes[NUM_NODES - NUM_LEAVES..NUM_NODES]
                           .iter_mut().rev()
                           .zip(leaves.iter_mut())
                           .enumerate()
        {
            *node = TreeNode::new_leaf(val as u16, last_group);
            *leaf = (NUM_NODES - 1 - val) as u16;
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
                    leaders[last_group as usize] = index as u16 + 1;
                    last_freq = freq;
                    last_group = groups.allocate();
                }
                *parent_node = TreeNode::new_branch(child_index as u16, freq, last_group);
            }
            tail_len -= parent_len;
            rest = head;
        }

        DynHuffTree {
            nodes,
            leaves,
            leaders,
            groups,
        }
    }

    #[inline(never)]
    fn rebuild_tree(&mut self) {
        // move leave values away maintaining order and dampen down frequency
        let mut node_filter = self.nodes.iter().filter(|n| n.is_leaf());
        let mut leave_nodes: [MaybeUninit<(IndexLeaf, u16)>; NUM_LEAVES] = unsafe {
            MaybeUninit::uninit().assume_init()
        };
        // let mut leave_nodes = [(IndexLeaf::new(), 0u16); NUM_LEAVES];
        for t in leave_nodes.iter_mut() {
            let node = node_filter.next().unwrap();
            *t = MaybeUninit::new((node.value, (node.freq + 1) / 2));
        }
        let leave_nodes = unsafe {
            mem::transmute::<_,[(IndexLeaf, u16); NUM_LEAVES]>(leave_nodes)
        };
        // an iterator of last leaf first
        let mut leaves_riter = leave_nodes.iter().rev();
        // get maybe next leaf
        let mut next_leaf = leaves_riter.next();
        let mut target_len = NUM_NODES;
        let mut nodes = &mut self.nodes[..];
        while nodes.len() > 2 {
            let child_index = nodes.len() - 1;
            // 1. copy at least 2 more leaves if not enough children
            let num_children = child_index + 1 - target_len;
            if num_children < 2 {
                for node in nodes[..target_len].iter_mut().rev().take(2 - num_children) {
                    let (value, freq) = next_leaf.unwrap();
                    let leaf_value = value.get_value() as usize;
                    target_len -= 1;
                    self.leaves[leaf_value] = target_len as u16;
                    node.value = *value;
                    node.freq = *freq;
                    next_leaf = leaves_riter.next();
                }
            }

            let (head, children) = nodes.split_at_mut(target_len);

            let branch_freq = children.iter().rev().take(2).map(|n| n.freq).sum();
            let mut target_mut = head.iter_mut().rev();

            // 2. copy more leaves until frequency is less
            while let Some((value, freq)) = next_leaf {
                if branch_freq < *freq {
                    break;
                }
                let leaf_value = value.get_value() as usize;
                let node = target_mut.next().unwrap();
                node.value = *value;
                node.freq = *freq;
                self.leaves[leaf_value] = target_mut.len() as u16;
                next_leaf = leaves_riter.next();
            }

            // 3. insert branch
            let mut node = target_mut.next().unwrap();
            node.make_branch(child_index as u16);
            node.freq = branch_freq;
            target_len = target_mut.len();
            for p in children.iter_mut().rev().take(2) {
                p.parent = target_len as u16;
            }
            // 4. repeat until root
            nodes = &mut nodes[..child_index - 1];
        }

        self.groups.reset();
        let mut group = self.groups.allocate();
        let mut freq = self.nodes[0].freq;
        self.nodes[0].group = group;
        self.leaders[group as usize] = 0;

        for (node, index) in self.nodes[1..].iter_mut().zip(1..) {
            if node.freq == freq {
                node.group = group;
            } else {
                freq = node.freq;
                group = self.groups.allocate();
                node.group = group;
                self.leaders[group as usize] = index;
            }
        }
    }

    #[inline(never)]
    fn promote_to_leader(&mut self, node_index: u16) -> u16 {
        let (node, head) = self.nodes[..node_index as usize + 1].split_last_mut().unwrap();
        let leader_index = self.leaders[node.group as usize];

        if leader_index == node_index {
            return node_index
        }

        let prev_leader = &mut head[leader_index as usize];
        let value = node.value;
        node.value = prev_leader.value;
        prev_leader.value = value;

        match node.value.as_type() {
            NodeType::Leaf(value) => {
                self.leaves[value as usize] = node_index;
            }
            NodeType::Branch(child_index) => {
                let child_index = child_index as usize;
                for child in self.nodes[child_index - 1..child_index + 1].iter_mut() {
                    child.parent = node_index;
                }
            }
        }

        match value.as_type() {
            NodeType::Leaf(value) => {
                self.leaves[value as usize] = leader_index;
            }
            NodeType::Branch(child_index) => {
                let child_index = child_index as usize;
                for child in self.nodes[child_index - 1..child_index + 1].iter_mut() {
                    child.parent = leader_index;
                }
            }
        }
        leader_index
    }

    #[inline(never)]
    fn increment_frequency(&mut self, node_index: u16) {
        let (head, tail) = self.nodes.split_at_mut(node_index as usize);
        let prev = head.last_mut().unwrap();
        let (node, tail) = tail.split_first_mut().unwrap();

        node.freq += 1;

        // node was part of the group with next nodes
        if let Some(next) = tail.first() {
            if node.group == next.group {
                // the next node is now a leader
                self.leaders[node.group as usize] += 1;
                if node.freq == prev.freq {
                    // join group of previous node
                    node.group = prev.group;
                }
                else {
                    // create own group
                    node.group = self.groups.allocate();
                    self.leaders[node.group as usize] = node_index;
                }

                return
            }
        }

        // node had its own group
        if node.freq == prev.freq {
            // join group of previous node
            self.groups.free(node.group);
            node.group = prev.group;
        }
    }

    #[inline(never)]
    fn increment_for_value(&mut self, value: u16) {
        // reorder tree on limit
        if self.nodes[0].freq >= REORDER_LIMIT {
            self.rebuild_tree();
        }

        self.nodes[0].freq += 1;

        let mut node_index = self.leaves[value as usize];
        // walk up from leaf and re-arrange nodes
        while node_index != 0 {
            node_index = self.promote_to_leader(node_index);
            self.increment_frequency(node_index);
            node_index = unsafe {
                self.nodes.get_unchecked(node_index as usize).parent
                // safe because tree was initialized in a sane way
            };
        }
    }

    pub fn read_entry<R: BitRead>(&mut self, mut entry: R) -> io::Result<u16> {
        let nodes = &self.nodes;
        let mut node = &nodes[0];
        loop {
            match node.value.as_type() {
                NodeType::Leaf(value) => {
                    self.increment_for_value(value);
                    return Ok(value)
                }
                NodeType::Branch(index) => {
                    let index = index as usize - entry.read_bits::<usize>(1)?;
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
            match node.value.as_type() {
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
    use super::*;

    #[test]
    fn dyntree_works() {
        println!("{}", core::mem::size_of::<IndexLeaf>());
        let mut tree = DynHuffTree::new();
        println!("{}", tree);
        for i in 0..NUM_LEAVES {
            println!("code: {:?}", i);
            for _ in 0..i + 1 {
                tree.increment_for_value(i as u16);
            }
        }
        for (i, node) in tree.nodes.iter().enumerate() {
            println!("{:3}: {:?} ({}) >{:3} ^{:3} g: {}", i,
                node.freq, node.value.get_leaf(), node.value.get_value(), node.parent, node.group);
        }
        for (i, leader) in tree.leaders.iter().enumerate() {
            println!("leader {}: {:?}", i, leader);
        }
        println!("{}", tree);
        // println!("leaf {:?}", tree.leaves);
        // println!("leader {:?}", tree.leaders);
    }
}
