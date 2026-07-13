// use core::num::NonZeroU16;
use bytemuck::{AnyBitPattern, NoUninit};
/// A packed tree entry object
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoUninit, AnyBitPattern)]
#[repr(transparent)]
pub struct TreeEntry(u16);

const LEAF_BIT: u16 = 1u16.rotate_right(1);

/// A tree entry node enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// Values are stored in leaves
    Leaf(u16),
    /// A branch holds an index to child nodes
    Branch(u16),
}

impl TreeEntry {
    /// The maximum tree node value or index
    pub const MAX_INDEX: usize = LEAF_BIT as usize - 1;

    /// Create a tree entry as a leaf with the given value
    #[inline]
    pub fn leaf(value: u16) -> TreeEntry {
        TreeEntry(value | LEAF_BIT)
    }

    /// Create a tree entry as a branch with the given child index
    #[inline]
    pub fn branch(child_index: usize) -> TreeEntry {
        TreeEntry((child_index as u16) & !LEAF_BIT)
    }

    /// Convert self to a node type enum
    #[inline]
    pub fn as_node(self) -> NodeType {
        let TreeEntry(entry) = self;
        let value = entry & !LEAF_BIT;
        if value == entry {
            NodeType::Branch(value)
        }
        else {
            NodeType::Leaf(value)
        }
    }

    /// Return whether this tree entry is a leaf
    #[allow(dead_code)]
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & LEAF_BIT == LEAF_BIT
    }

    #[cfg(feature = "lh1")]
    #[inline]
    pub(crate) fn set_as_branch(&mut self, child_index: usize) {
        self.0 = (child_index as u16) & !LEAF_BIT;
    }
    /// Return a node value regardless of the node type
    #[inline]
    pub(crate) fn as_value(self) -> u16 {
        self.0 & !LEAF_BIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_entry_works() {
        assert!(!TreeEntry::branch(0x7FFF).is_leaf());
        assert_eq!(size_of::<TreeEntry>(), 2);
        assert_eq!(LEAF_BIT, 0x8000);
        let leaf0 = TreeEntry::leaf(0);
        assert!(leaf0.is_leaf());
        assert_eq!(leaf0, TreeEntry::leaf(0x8000));
        assert_eq!(leaf0.as_value(), 0);
        assert_eq!(leaf0.as_node(), NodeType::Leaf(0));
        let leaf1 = TreeEntry::leaf(1);
        assert!(leaf1.is_leaf());
        assert_eq!(leaf1, TreeEntry::leaf(0x8001));
        assert_eq!(leaf1.as_value(), 1);
        assert_eq!(leaf1.as_node(), NodeType::Leaf(1));
        let branch0 = TreeEntry::branch(0);
        let branch1 = TreeEntry::branch(0x7fff);
        assert!(!branch0.is_leaf());
        assert_eq!(branch0.as_value(), 0);
        assert_eq!(branch0.as_node(), NodeType::Branch(0));
        assert!(!branch1.is_leaf());
        assert_eq!(branch1.as_value(), 0x7fff);
        assert_eq!(branch1.as_node(), NodeType::Branch(0x7fff));
        assert_eq!(branch0.as_node(), NodeType::Branch(0));
        assert_eq!(branch1.as_node(), NodeType::Branch(0x7fff));
        assert_eq!(TreeEntry::branch(0x8000).as_node(), NodeType::Branch(0));

        #[cfg(feature = "lh1")]
        {
            let mut leaf0 = leaf0;
            leaf0.set_as_branch(0);
            assert!(!leaf0.is_leaf());
            assert_eq!(leaf0.as_node(), NodeType::Branch(0));
            leaf0.set_as_branch(0x8000);
            assert_eq!(leaf0.as_node(), NodeType::Branch(0));
            leaf0.set_as_branch(0x7fff);
            assert_eq!(leaf0.as_node(), NodeType::Branch(0x7fff));
        }
    }
}
