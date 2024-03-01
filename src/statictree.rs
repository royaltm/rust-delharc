/*! # Static Huffman Coding.

In the following example, letters represent leaves and numbers represent branches.
Branch numbers indicate their positions in a vector in which the tree is being stored.

```text
     0
   /   \
  a     2
      /   \
     3     4
   /  \   /  \
  b    c 7    8
        / \  /  \
       d  e 11   12
           / \   / \
          f   g h   i
```

The above tree can be built from the following `lengths`:

```text
a -> 1
b -> 3
c -> 3
d -> 4
e -> 4
f -> 5
g -> 5
h -> 5
i -> 5
```

When reading, the following bit paths will result in finding the particular leaves:

```text
0     -> a
100   -> b
101   -> c
1100  -> d
1101  -> e
11100 -> f
11101 -> g
11110 -> h
11111 -> i
```
*/
use core::fmt;
use crate::error::LhaError;
use crate::bitstream::BitRead;
#[cfg(not(feature = "std"))]
use alloc::{vec::Vec, string::String};

pub mod entry;
use entry::*;

/// A static Huffman tree.
#[derive(Debug, Clone)]
pub struct HuffTree {
    tree: Vec<TreeEntry>
}

impl HuffTree {
    /// Creates a new and empty `HuffTree` with the reserved node capacity.
    ///
    /// Any attempt to read from a new tree will result in panic.
    pub fn with_capacity(capacity: usize) -> Self {
        let tree = Vec::with_capacity(capacity);
        HuffTree { tree }
    }
    /// Initializes a `HuffTree` in such a way that any attept to read from it will always
    /// result in the given value, without even reading any position bits.
    pub fn set_single(&mut self, value: u16) {
        self.tree.clear();
        self.tree.push(TreeEntry::leaf(value));
    }
    /// Builds the tree from the given array of lengths.
    ///
    /// Each entry's index represents the `value` stored in tree leaves. Each entry's content represents
    /// the `length` (or depth), measured in nodes from the tree root, at which the leaf is being created.
    ///
    /// * Entries containing `0` are being ignored.
    /// * If too many entries contain the same `length`, exceeding the given `length` capacity, an error
    ///   is being returned.
    /// * If the size of the argument slice is larger than or equal to the [TreeEntry::MAX_INDEX] / 2,
    ///   an error is being returned.
    /// * If the number of created nodes would exceed [TreeEntry::MAX_INDEX], an error is being returned.
    /// * An error is returned if a built tree is incomplete.
    pub fn build_tree(&mut self, value_lengths: &[u8]) -> Result<(), &'static str> {
        // println!("({}) {:?}", value_lengths.len(), value_lengths);
        if value_lengths.len() > TreeEntry::MAX_INDEX / 2 {
            return Err("too many code lengths");
        }
        let tree = &mut self.tree;

        tree.clear();
        // the number of allocated tree indices
        // the tree size should be equal to the value of this variable
        let mut max_allocated: usize = 1; // start with a single (root) node
        for current_len in 1u8.. {
            // add missing branches
            for _ in  tree.len()..max_allocated {
                match TreeEntry::branch(max_allocated) {
                    Ok(branch) => tree.push(branch),
                    Err(e) => {
                        // make sure no outstanding branch indices exist
                        tree.clear();
                        return Err(e);
                    }
                }
                // for every branch node, two new child nodes are required
                max_allocated += 2;
            }
            // fill tree with leaves found in the lengths table at the current length
            let more_leaves = value_lengths.iter().copied().zip(0..)
                              .fold(false, |mut more, (len, value)| {
                if len == current_len {
                    tree.push(TreeEntry::leaf(value));
                }
                else if len > current_len {
                    // there are more leaves to process
                    more = true;
                }
                more
            });
            if tree.len() > max_allocated {
                return Err("too many leaves");
            }
            if !more_leaves {
                break;
            }
        }
        // println!("tree missing leaves: {}", max_allocated - tree.len());
        if tree.len() != max_allocated {
            return Err("missing some leaves")
        }
        // // make sure no outstanding indices exist, perhaps this should be reported as an error
        // tree.extend(
        //     (tree.len()..max_allocated).map(|_| TreeEntry::leaf(0) )
        // );
        Ok(())
    }
    /// Returns the `value` of the leaf by following the bit `path` read from the given bit reader.
    ///
    /// Bits are being read from the stream until a leaf is being encountered. The `value` stored in that
    /// leaf is being returned.
    ///
    /// If a branch is encountered a bit of value `0` indicates that the left node should be followed,
    /// and `1` to take the path to the right.
    ///
    /// If a tree has been initialized with [HuffTree::set_single] this method will always return the
    /// single `value`, without reading any bits from the stream.
    ///
    /// # Panics
    /// Panics if a tree has not been built or otherwise initialized as a single value tree.
    pub fn read_entry<R: BitRead>(&self, mut path: R) -> Result<u16, LhaError<R::Error>> {
        let tree = &self.tree;
        let mut node = &tree[0]; // panics if tree uninitialized
        loop {
            match node.as_type() {
                NodeType::Leaf(code) => return Ok(code),
                NodeType::Branch(index) => {
                    let index = index as usize + path.read_bits::<usize>(1)?;
                    debug_assert!(index < tree.len());
                    node = unsafe {
                        // safe because tree was initialized in a sane way,
                        // no outstanding child index has been used
                        tree.get_unchecked(index)
                    };
                }
            }
        }
    }
}

impl fmt::Display for HuffTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        fn fmt_step(tree: &Vec<TreeEntry>, index: usize, f: &mut fmt::Formatter<'_>, prefix: &mut String) -> fmt::Result {
            match tree[index].as_type() {
                NodeType::Leaf(code) => writeln!(f, "{} -> {}", prefix, code)?,
                NodeType::Branch(index) => {
                    prefix.push('0');
                    fmt_step(tree, index as usize, f, prefix)?;
                    prefix.pop();
                    prefix.push('1');
                    fmt_step(tree, index as usize + 1, f, prefix)?;
                    prefix.pop();
                }
            }
            Ok(())
        }

        if !self.tree.is_empty() {
            let mut prefix = String::new();
            fmt_step(&self.tree, 0, f, &mut prefix)?;
        }
        Ok(())
    }
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {
    use crate::bitstream::BitStream;
    use std::collections::{HashSet, HashMap};
    use super::*;

    fn validate_tree(tree: &HuffTree, num_leaves: usize) {
        let mut leaves: HashMap<u16, usize> = HashMap::with_capacity(num_leaves);
        let mut children: HashSet<u16> = HashSet::with_capacity(tree.tree.len());
        for (index, node) in tree.tree.iter().enumerate() {
            match node.as_type() {
                NodeType::Leaf(value) => {
                    // all leaves should be unique
                    assert!(leaves.insert(value, index).is_none());
                }
                NodeType::Branch(child_index) => {
                    // invalid (default) node should not be present
                    assert!(child_index != 0);
                    // child_index should not exceed the tree length
                    assert!((child_index as usize) < tree.tree.len() - 1);
                    // all child indexes should be odd
                    assert!(child_index & 1 == 1);
                    // there must be no duplicate parents of the same children
                    assert!(children.insert(child_index));
                }
            }
        }
        // all leaves should be present
        assert_eq!(leaves.len(), num_leaves);
        // all leaves should be reachable and on the unique path
        fn into_branch(nodes: &[TreeEntry], index: usize, leaves: &mut HashSet<u16>) {
            match nodes[index].as_type() {
                NodeType::Leaf(code) => {
                    assert!(leaves.insert(code));
                }
                NodeType::Branch(index) => {
                    into_branch(nodes, index as usize, leaves);
                    into_branch(nodes, index as usize + 1, leaves);
                }
            }
        }
        let mut leaves: HashSet<u16> = HashSet::with_capacity(num_leaves);
        into_branch(&tree.tree, 0, &mut leaves);
        assert_eq!(leaves.len(), num_leaves);
    }

    #[test]
    fn hufftree_works() {
        let mut tree = HuffTree::with_capacity(0);
        println!("{}", tree);
        tree.set_single(42);
        validate_tree(&tree, 1);
        let path = BitStream::new([].as_ref());
        assert_eq!(tree.read_entry(path).unwrap(), 42);
        println!("{}", tree);

        tree.build_tree(&[0, 1, 0, 1]).unwrap();
        validate_tree(&tree, 2);
        println!("{}", tree);

        tree.build_tree(&[0, 0, 0, 1, 0, 3, 3, 0, 4, 4, 5, 0, 0, 5, 5, 5]).unwrap();
        println!("{}", tree);
        validate_tree(&tree, 9);
        assert_eq!(tree.tree.len(), 9 + 8);
        let bits: &[u8] = &[0b01001011, 0b10011011, 0b11001110, 0b11111011, 0b11100000];
        let mut path = BitStream::new(bits);
        let mut res = Vec::new();
        for _ in 0..9 {
            res.push(tree.read_entry(path.by_ref()).unwrap());
        }
        assert_eq!(res, [3, 5, 6, 8, 9, 10, 13, 14, 15]);

        assert!(tree.build_tree(&[0, 1, 0, 1, 1]).is_err());
        assert!(tree.build_tree(&[0, 1, 0, 1, 10]).is_err());
    }
}
