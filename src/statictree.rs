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
#![allow(dead_code)]
#[cfg(test)]
use core::fmt;
use core::cmp::Ordering;
use crate::{bitstream::BitRead, error::LhaError};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(all(test, not(feature = "std")))]
use alloc::string::String;

pub mod entry;
use entry::*;

/// A static Huffman tree.
#[derive(Debug, Clone)]
pub struct HuffTree {
    tree: Vec<TreeEntry>
}

impl HuffTree {
    /// Creates a new and empty [`HuffTree`] without allocating anything.
    ///
    /// Any attempt to read from a new tree will result in a panic.
    pub fn new() -> Self {
        let tree = Vec::new();
        HuffTree { tree }
    }
    /// Creates a new and empty `HuffTree` with the reserved node capacity.
    ///
    /// Any attempt to read from a new tree will result in a panic.
    pub fn with_capacity(capacity: usize) -> Self {
        let tree = Vec::with_capacity(capacity);
        HuffTree { tree }
    }
    /// Attempt to reserve enough memory to build a tree from the given number of leaves.
    pub fn try_reserve(&mut self, num_leaves: usize) -> Result<(), &'static str> {
        if num_leaves > TreeEntry::MAX_INDEX / 2 {
            return Err("too many leaves");
        }
        if num_leaves == 0 {
            return Ok(())
        }
        let required_size = num_leaves * 2 - 1;
        if let Some(reserve) = required_size.checked_sub(self.tree.len())
            && reserve != 0
        {
            self.tree.try_reserve_exact(reserve).map_err(|_| "not enough memory")
        }
        else {
            Ok(())
        }
    }
    /// Clears the tree from all nodes.
    ///
    /// Any attempt to read from tree after a call to this function will result in a panic.
    pub fn clear(&mut self) {
        self.tree.clear();
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
    /// * If the size of the argument slice is larger than or equal to the [`TreeEntry::MAX_INDEX`] / 2,
    ///   an error is being returned.
    /// * If the number of created nodes would exceed [`TreeEntry::MAX_INDEX`], an error is being returned.
    /// * An error is returned if a built tree is incomplete.
    pub fn build_tree(&mut self, value_lengths: &[u8]) -> Result<(), &'static str> {
        let tree = &mut self.tree;
        tree.clear();

        // println!("({}) {:?}", value_lengths.len(), value_lengths);
        if value_lengths.len() > TreeEntry::MAX_INDEX / 2 {
            return Err("too many code lengths");
        }

        // the number of allocated tree indices
        // the tree size should be equal to the value of this variable
        let mut max_allocated: usize = 1; // start with a single (root) node
        for current_len in 1u8..=u8::MAX {
            // add missing branches
            let max_limit = max_allocated;
            for _ in  tree.len()..max_limit {
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
                match len.cmp(&current_len) {
                    Ordering::Equal => {
                        tree.push(TreeEntry::leaf(value));
                    }
                    Ordering::Greater => {
                        // there are more leaves to process
                        more = true;
                    }
                    Ordering::Less => {}
                }
                more
            });
            if tree.len() > max_allocated {
                tree.clear(); // for consistency
                return Err("too many leaves");
            }
            if !more_leaves {
                break;
            }
        }
        // println!("tree missing leaves: {}", max_allocated - tree.len());
        if tree.len() != max_allocated {
            tree.clear(); // make sure no outstanding branch indices exist
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
    /// If a tree has been initialized with [`HuffTree::set_single`] this method will always return the
    /// single `value`, without reading any bits from the stream.
    ///
    /// # Panics
    /// Panics if a tree has not been built or otherwise initialized as a single value tree.
    pub fn read_entry<R: BitRead>(&self, mut path: R) -> Result<u16, LhaError<R::Error>> {
        let tree = &self.tree;
        let mut node = &tree[0]; // panics if tree uninitialized
        loop {
            match node.as_node() {
                NodeType::Leaf(code) => return Ok(code),
                NodeType::Branch(index) => {
                    let index = index as usize + path.read_bits::<usize>(1)?;
                    debug_assert!(index < tree.len());
                    node = unsafe {
                        // SAFETY: safe because tree was initialized in a sane way,
                        // no outstanding child index has been used
                        tree.get_unchecked(index)
                    };
                }
            }
        }
    }
    /// Return whether the tree is empty (uninitialized).
    pub fn is_empty(&self) -> bool {
        self.tree.is_empty()
    }
    /// Return the number of populated nodes.
    pub fn len(&self) -> usize {
        self.tree.len()
    }
    /// Return a reference to a collection of tree nodes
    pub fn inspect(&self) -> &[TreeEntry] {
        &self.tree
    }
    /// Shrinks the capacity of the tree as much as possible.
    pub fn shrink_to_fit(&mut self) {
        self.tree.shrink_to_fit()
    }
}

#[cfg(test)]
impl fmt::Display for HuffTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        fn fmt_step(tree: &[TreeEntry], index: usize, f: &mut fmt::Formatter<'_>, prefix: &mut String) -> fmt::Result {
            match tree[index].as_node() {
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
    use std::collections::{HashSet, HashMap};
    use rand::{RngExt, Rng, RngReader, seq::SliceRandom};
    use crate::bitstream::BitStream;
    use super::*;

    fn validate_tree(tree: &HuffTree, num_leaves: usize) {
        let mut leaves: HashMap<u16, usize> = HashMap::with_capacity(num_leaves);
        let mut children: HashSet<u16> = HashSet::with_capacity(tree.tree.len());
        for (index, node) in tree.tree.iter().enumerate() {
            match node.as_node() {
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
            match nodes[index].as_node() {
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
        let mut tree = HuffTree::new();
        println!("{}", tree);
        tree.set_single(42);
        validate_tree(&tree, 1);
        let path = BitStream::new([].as_ref());
        assert_eq!(tree.read_entry(path).unwrap(), 42);
        println!("{}", tree);

        tree.build_tree(&[0, 1, 0, 1]).unwrap();
        validate_tree(&tree, 2);
        assert_eq!(tree.tree.len(), 3);
        println!("{}", tree);
        let bits: &[u8] = &[0b01110001];
        let mut path = BitStream::new(bits);
        let mut res = Vec::new();
        for _ in 0..8 {
            res.push(tree.read_entry(&mut path).unwrap());
        }
        assert_eq!(res, [1,3,3,3,1,1,1,3]);

        tree.build_tree(&[1,2,3,4,5,6,7,8,0,0,0,9,9]).unwrap();
        validate_tree(&tree, 10);
        assert_eq!(tree.tree.len(), 10+9);
        println!("{}", tree);
        let bits: u64 = 0b0_10_110_1110_11110_111110_1111110_11111110_111111110_111111111 << 10;
        let bits = bits.to_be_bytes();
        let mut path = BitStream::new(bits.as_slice());
        let mut res = Vec::new();
        for _ in 0..10 {
            res.push(tree.read_entry(&mut path).unwrap());
        }
        assert_eq!(res, [0,1,2,3,4,5,6,7,11,12]);

        assert!(!tree.is_empty());
        assert_ne!(tree.len(), 0);
        assert_ne!(tree.tree.capacity(), 0);
        tree.clear();
        assert!(tree.is_empty());
        assert_ne!(tree.tree.capacity(), 0);
        tree.shrink_to_fit();
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.tree.capacity(), 0);
        tree.try_reserve(0).unwrap();
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.tree.capacity(), 0);
        let lengths = [0, 0, 0, 1, 0, 3, 3, 0, 4, 4, 5, 0, 0, 5, 5, 5];
        tree.try_reserve(9).unwrap();
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.tree.capacity(), 9 + 8);
        tree.build_tree(&lengths).unwrap();
        println!("{}", tree);
        validate_tree(&tree, 9);
        assert_eq!(tree.len(), 9 + 8);
        assert_eq!(tree.tree.capacity(), 9 + 8);
        let bits: &[u8] = &[0b01001011, 0b10011011, 0b11001110, 0b11111011, 0b11100000];
        let mut path = BitStream::new(bits);
        let mut res = Vec::new();
        for _ in 0..9 {
            res.push(tree.read_entry(&mut path).unwrap());
        }
        assert_eq!(res, [3, 5, 6, 8, 9, 10, 13, 14, 15]);

        let mut rng = rand::rng();
        let mut rndstream = BitStream::new(RngReader(&mut rng));
        for _ in 0..1_000_000 {
            let value = tree.read_entry(&mut rndstream).unwrap();
            assert!(matches!(value, 3|5|6|8|9|10|13|14|15), "unexpected value returned: {}", value);
        }

        assert_eq!(tree.build_tree(&[]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.inspect(), &[]);
        assert_eq!(tree.build_tree(&[0]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.build_tree(&[0, 0, 0]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.build_tree(&[0, 0, 0, 1]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.build_tree(&[0, 1, 0, 1, 1]).unwrap_err(), "too many leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.build_tree(&[0, 1, 0, 1, 10]).unwrap_err(), "too many leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.build_tree(&[0, 1, 0, 2, 5]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        let code_length = vec![0u8; 0x4000-1];
        assert_eq!(tree.build_tree(&code_length).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
        let code_length = vec![0u8; 0x4000];
        assert_eq!(tree.try_reserve(0x4000).unwrap_err(), "too many leaves");
        assert_eq!(tree.build_tree(&code_length).unwrap_err(), "too many code lengths");
        assert!(tree.is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    #[ignore = "long tests"]
    fn hufftree_long_tests() {
        let mut tree = HuffTree::with_capacity(32768);

        // build a random tree lengths with an upper num of values
        fn build_random_lengths(max_values: usize, rng: &mut impl Rng, out: &mut Vec<u8>) -> usize {
            out.clear();
            let mut max_leaves = 2usize;
            let mut last_level = u8::MAX;
            let mut iter = 1..u8::MAX;
            for level in iter.by_ref() {
                let n = out.len();
                let remaining = max_values - n;
                let num_leaves;
                if let Some(margin) = (max_leaves * 2).checked_sub(remaining)  {
                    if remaining <= max_leaves {
                        last_level = level;
                        break
                    }
                    num_leaves = margin;
                }
                else {
                    num_leaves = rng.random_range(0..max_leaves);
                };
                max_leaves = (max_leaves - num_leaves) * 2;
                out.resize(n + num_leaves, level);
            }
            out.resize(out.len() + max_leaves, last_level);
            out.len()
        }

        let mut rng = rand::rng();
        let vec = &mut Vec::new();
        for nvalues in (2..=10).chain([20,50,100,200,256,0x3FFF]) {
            for i in 0..(nvalues*2).max(1).min(if nvalues <= 256 { 20 } else { 50 }) {
                println!("-------------- [{}][{}]", nvalues, i + 1);
                let nleaves = build_random_lengths(nvalues, &mut rng, vec);
                // println!("leaves: {}", nleaves);
                if nvalues <= 256 {
                    assert_eq!(nvalues, nleaves);
                }
                tree.build_tree(vec).unwrap();
                if nvalues < 100 {
                    // println!("{}", tree);
                }
                validate_tree(&tree, nleaves);
                let mut rndstream = BitStream::new(RngReader(&mut rng));
                for _ in 0..1_000_000 {
                    let value = tree.read_entry(&mut rndstream).unwrap() as usize;
                    assert!(value < nvalues, "unexpected value returned: {}", value);
                }

                let max_level = vec.iter().copied().max().unwrap();
                vec.push(rng.random_range(1..=max_level));
                assert_eq!(tree.build_tree(vec).unwrap_err(), if vec.len() > 0x3FFF {
                    "too many code lengths"
                }
                else {
                    "too many leaves"
                });
                vec.pop();
                let last = vec.pop().unwrap();
                assert_eq!(tree.build_tree(vec).unwrap_err(), "missing some leaves");
                vec.push(last);

                let nleaves = vec.len();
                vec.resize(0x3FFF, 0);
                vec.shuffle(&mut rng);
                tree.build_tree(vec).unwrap();
                if nvalues < 100 {
                    // println!("{}", tree);
                }
                validate_tree(&tree, nleaves);
                let mut rndstream = BitStream::new(RngReader(&mut rng));
                for _ in 0..1_000_000 {
                    let value = tree.read_entry(&mut rndstream).unwrap() as usize;
                    assert!(value < vec.len() && vec[value] != 0, "unexpected value returned: {}", value);
                }
            }
        }
    }
}
