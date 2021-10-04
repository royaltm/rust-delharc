#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct TreeEntry(u16);

const LEAF_BIT: u16 = 1u16.rotate_right(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Leaf(u16),
    Branch(u16),
}

impl TreeEntry {
    pub const MAX_INDEX: usize = LEAF_BIT as usize - 1;

    #[inline]
    pub fn leaf(value: u16) -> TreeEntry {
        TreeEntry(value | LEAF_BIT)
    }

    #[inline]
    pub fn branch(child_index: usize) -> Result<TreeEntry, &'static str> {
        if child_index > Self::MAX_INDEX {
            return Err("tree index out of range");
        }
        Ok(TreeEntry(child_index as u16))
    }

    #[inline]
    pub fn as_type(self) -> NodeType {
        let TreeEntry(entry) = self;
        let value = entry & !LEAF_BIT;
        if value == entry {
            NodeType::Branch(value)
        }
        else {
            NodeType::Leaf(value)
        }
    }

    #[allow(dead_code)]
    #[inline]
    pub fn set_as_branch(&mut self, child_index: usize) {
        self.0 = (child_index as u16) & !LEAF_BIT;
    }

    #[allow(dead_code)]
    #[inline]
    pub fn is_leaf(self) -> bool {
        self.0 & LEAF_BIT == LEAF_BIT
    }

    #[allow(dead_code)]
    #[inline]
    pub fn as_value(self) -> u16 {
        self.0 & !LEAF_BIT
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;
    #[test]
    fn tree_entry_works() {
        assert_eq!(mem::size_of::<TreeEntry>(), 2);
        assert_eq!(LEAF_BIT, 0x8000);
        let mut leaf0 = TreeEntry::leaf(0);
        assert!(leaf0.is_leaf());
        assert_eq!(leaf0, TreeEntry::leaf(0x8000));
        assert_eq!(leaf0.as_value(), 0);
        let leaf1 = TreeEntry::leaf(1);
        assert!(leaf1.is_leaf());
        assert_eq!(leaf1, TreeEntry::leaf(0x8001));
        assert_eq!(leaf1.as_value(), 1);
        let branch0 = TreeEntry::branch(0).unwrap();
        let branch1 = TreeEntry::branch(0x7fff).unwrap();
        assert!(!branch0.is_leaf());
        assert_eq!(branch0.as_value(), 0);
        assert!(!branch1.is_leaf());
        assert_eq!(branch1.as_value(), 0x7fff);
        assert!(TreeEntry::branch(0x8000).is_err());
        assert_eq!(leaf0.as_type(), NodeType::Leaf(0));
        assert_eq!(leaf1.as_type(), NodeType::Leaf(1));
        assert_eq!(branch0.as_type(), NodeType::Branch(0));
        assert_eq!(branch1.as_type(), NodeType::Branch(0x7fff));
        leaf0.set_as_branch(0);
        assert!(!leaf0.is_leaf());
        assert_eq!(leaf0.as_type(), NodeType::Branch(0));
        leaf0.set_as_branch(0x8000);
        assert_eq!(leaf0.as_type(), NodeType::Branch(0));
        leaf0.set_as_branch(0x7fff);
        assert_eq!(leaf0.as_type(), NodeType::Branch(0x7fff));
    }
}
