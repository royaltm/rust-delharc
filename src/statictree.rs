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
use core::cmp::Ordering;
use crate::{bitstream::BitRead, error::LhaError};
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;
#[cfg(all(test, not(feature = "std")))]
use alloc::string::String;
#[cfg(test)]
use core::fmt;

mod entry;
pub use entry::*;

/// A static Huffman Tree.
#[derive(Debug, Clone)]
pub struct HuffTree {
    tree: Vec<TreeEntry>
}

impl Default for HuffTree {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

macro_rules! unsafe_assert {
    ($expr:expr) => {
        #[cfg(all(not(feature = "no-unsafe-assertions"), not(debug_assertions)))]
        unsafe {
            core::hint::assert_unchecked($expr)
        }
        debug_assert!($expr)
    };
}

impl HuffTree {
    /// Creates a new and empty [`HuffTree`] without allocating anything.
    ///
    /// Any attempt to read from a new tree will result in a panic.
    #[inline]
    pub fn new() -> Self {
        let tree = Vec::new();
        HuffTree { tree }
    }
    /// Creates a new and empty [`HuffTree`] with the reserved node capacity.
    ///
    /// Any attempt to read from a new tree will result in a panic.
    #[inline]
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
    /// Initializes a [`HuffTree`] in such a way that any attept to read from it will always
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
    /// * Entries containing `0` are being ignored - there will be no leaf with a `value` from such an index.
    /// * If the size of the argument slice is larger than [`TreeEntry::MAX_INDEX`] / 2, an error is returned.
    /// * If too many entries contain the same `length`, exceeding the given `length` capacity, an error is
    ///   returned.
    /// * An error is returned if a tree is incomplete - there are not enough leaves to fill the last length.
    ///
    /// The root of the tree (`length = 0`) is always a branch. The maximum number of leaves on the first
    /// length is 2. If there are 2 leaves on the first length, no more leaves can be added to the tree.
    /// The number of leaves on each next length depends on the number of leaves added on smaller lengths.
    ///
    /// # Features
    /// With the `fast-static-tree` feature this method forwards to [`Self::build_tree_with_sort`],
    /// or [`Self::build_tree_simple`] if the feature is not present.
    #[inline(always)]
    pub fn build_tree(&mut self, value_lengths: &[u8]) -> Result<(), &'static str> {
        #[cfg(not(feature = "fast-static-tree"))]
        {
            self.build_tree_simple(value_lengths)
        }
        #[cfg(feature = "fast-static-tree")]
        {
            self.build_tree_with_sort(value_lengths)
        }
    }
    /// Builds the tree from the given array of lengths.
    ///
    /// See [`Self::build_tree`].
    ///
    /// This algorithm sorts leaves by depth to avoid multiple passes of the code length table.
    /// Adding unstable sort algorithm increases code size by a few kilobytes.
    ///
    /// The time complexity is `O(n) + O(v * log(v))` where `v` is the number of populated values (leaves),
    /// and `n` is the size of `value_lengths`.
    #[cfg(feature = "fast-static-tree")]
    #[cfg_attr(docsrs, doc(cfg(feature = "fast-static-tree")))]
    pub fn build_tree_with_sort(&mut self, value_lengths: &[u8]) -> Result<(), &'static str> {
        let tree_vec = &mut self.tree;
        tree_vec.clear();

        // println!("({}) {:?}", value_lengths.len(), value_lengths);
        if value_lengths.len() > TreeEntry::MAX_INDEX / 2 {
            return Err("too many code lengths");
        }

        // step 1: add leaves with values corresponding to value_lengths indexes O(n)
        for (&depth, value) in value_lengths.iter().zip(0u16..) {
            if depth != 0 {
                tree_vec.push(TreeEntry::leaf(value));
            }
        }
        let num_leaves = tree_vec.len();
        if num_leaves <= 1 {
            tree_vec.clear();
            return Err("missing some leaves")
        }

        // step 2: make room for branches before leaves O(n)
        let last_leaf = tree_vec.pop().unwrap();
        tree_vec.extend_from_within(..);
        tree_vec.push(last_leaf); // maintain partial order (by value)

        // step 3: sort leaves by depth and value O(v * log(v))
        // return leaf depth indexed by its value
        let get_length = |value: u16| -> u32 {
            let index = usize::from(value);
            // SAFETY: get_length must only be called for leaf values created at step 1
            unsafe_assert!(index < value_lengths.len());
            value_lengths[index].into()
        };
        // freeze vec
        let tree = tree_vec.as_mut_slice();
        // SAFETY: remind compiler the relation between num_leaves and tree.len()
        // this is the state after extend_from_within
        unsafe_assert!(num_leaves < tree.len());
        let leaf_index = tree.len() - num_leaves; // target leave index
        // sort leaves by depth and value
        tree[leaf_index..].sort_unstable_by_key(|n: &TreeEntry| -> u32 {
            let value = n.as_value();
            (get_length(value) << 16) | u32::from(value)
        });

        // step 4: add branches and populate leaves at certain tree depths O(T)
        let mut leaf_index = leaf_index; // first source leave index
        let mut node_index = 0; // first target node index
        let mut max_allocated: usize = 1; // start with a single (root) node
        'depth: for current_len in 1u32.. {
            // add missing branches
            let end_index = max_allocated;
            // SAFETY: initial max_allocated is 1 < tree.len() (min 2)
            // max_allocated, after updating, is validated with max_allocated <= leaf_index
            // condition before the next iteration.
            // In reality the inequality is sharper, because the last branch is followed by leaves.
            // node_index < max_allocated because it can only be changed in the range ..max_allocated.
            unsafe_assert!(node_index < end_index && end_index <= tree.len());
            for branch in tree[node_index..end_index].iter_mut() {
                *branch = TreeEntry::branch(max_allocated);
                // for every branch node, two new child nodes are required
                max_allocated += 2;
            }
            // SAFETY: max_allocated can not outgrow tree.len() because
            // number of leaves is always > number of branches by definition
            unsafe_assert!(max_allocated <= tree.len());
            node_index = end_index;
            // add all leaves at the current depth
            // the last iteration here should be in leaf_index..tree.len() range
            for i in node_index..max_allocated {
                // SAFETY: end_index (previous max_allocated) <= leaf_index (condition below)
                //         max_allocated <= tree.len()
                // thus leaf_index can not outgrow the tree.len()
                unsafe_assert!(leaf_index < tree.len());
                let leaf = tree[leaf_index];
                if get_length(leaf.as_value()) == current_len {
                    tree[i] = leaf;
                    leaf_index += 1;
                }
                else if max_allocated > leaf_index {
                    tree_vec.clear(); // make sure no outstanding branch indices exist
                    return Err("missing some leaves")
                }
                else {
                    // leaves are sorted so lengths can only go up
                    node_index = i;
                    // SAFETY: node_index < max_allocated && max_allocated <= leaf_index
                    continue 'depth
                }
            }
            if max_allocated != tree.len() {
                tree_vec.clear(); // for consistency
                return Err("too many leaves");
            }
            break
        }
        Ok(())
    }
    /// Builds the tree from the given array of lengths.
    ///
    /// See [`Self::build_tree`].
    ///
    /// This naive implementation iterates the argument slice as many times as the deepest leaf
    /// length, but it produces very small code.
    ///
    /// The time complexity is `O(l*n)` where `l` is the highest leaf depth and `n` is the size of
    //  the `value_lengths` slice.
    pub fn build_tree_simple(&mut self, value_lengths: &[u8]) -> Result<(), &'static str> {
        let tree = &mut self.tree;
        tree.clear();

        // println!("({}) {:?}", value_lengths.len(), value_lengths);
        if value_lengths.len() > TreeEntry::MAX_INDEX / 2 {
            return Err("too many code lengths");
        }

        // the number of allocated tree indices
        let mut max_allocated: usize = 1; // start with a single (root) node
        let mut max_nodes: usize = 3; // start with a 3 node tree
        for current_len in 1u8..=u8::MAX {
            // add missing branches
            let more_branches = max_allocated - tree.len();
            for _ in  0..more_branches {
                if max_allocated > max_nodes { // too many branches created
                    // println!("too many!!! max nodes: {} max_allocated: {} tree: {}", max_nodes, max_allocated, tree.len());
                    tree.clear();
                    return Err("missing some leaves"); 
                }
                tree.push(TreeEntry::branch(max_allocated));
                // for every branch node, two new child nodes are required
                max_allocated += 2;
            }
            // fill tree with leaves found in the lengths table at the current length
            let more_leaves = value_lengths.iter().copied().zip(0u16..)
                              .fold(0, |mut more, (len, value)| {
                match len.cmp(&current_len) {
                    Ordering::Equal => {
                        tree.push(TreeEntry::leaf(value));
                    }
                    Ordering::Greater => {
                        // there are more leaves to process
                        more += 1;
                    }
                    Ordering::Less => {}
                }
                more
            });

            if tree.len() > max_allocated {
                tree.clear(); // for consistency
                return Err("too many leaves");
            }

            if more_leaves == 0 {
                break;
            }

            max_nodes = (max_allocated + more_leaves).min(TreeEntry::MAX_INDEX);
        }
        if tree.len() != max_allocated {
            // println!("tree missing leaves: {}", max_allocated - tree.len());
            tree.clear(); // make sure no outstanding branch indices exist
            return Err("missing some leaves")
        }
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
                    let is_one = path.read_bit()?;
                    let index = usize::from(index);
                    // SAFETY: branches must only point to valid children indexes
                    unsafe_assert!(index < tree.len() - 1);
                    node = if is_one {
                        &tree[index + 1]
                    }
                    else {
                        &tree[index]
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
        assert_eq!(tree.build_tree(&[1]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[255]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[255;5]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[255;0x3FFF]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[1,1,1]).unwrap_err(), "too many leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[1,1,1]).unwrap_err(), "too many leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[3,3,3,1]).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[1,3,3,3,3,1]).unwrap_err(), "too many leaves");
        assert!(tree.is_empty());
        assert_eq!(tree.build_tree(&[1,3,3,3,3,3]).unwrap_err(), "too many leaves");
        assert!(tree.is_empty());
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

        let mut code_length = Vec::with_capacity(0x4000);
        for i in 1u8..=255u8 {
            code_length.push(i);
        }
        code_length.shuffle(&mut rng);
        assert_eq!(tree.build_tree(&code_length).unwrap_err(), "missing some leaves");
        assert!(tree.is_empty());
        code_length.push(255);
        code_length.shuffle(&mut rng);
        tree.shrink_to_fit();
        tree.try_reserve(256).unwrap();
        tree.build_tree(&code_length).unwrap();
        assert_eq!(tree.tree.len(), 256+255);
        assert_eq!(tree.tree.capacity(), 256+255);
        validate_tree(&tree, 256);
        let mut rng = rand::rng();
        let mut rndstream = BitStream::new(RngReader(&mut rng));
        for _ in 0..1_000_000 {
            let value = tree.read_entry(&mut rndstream).unwrap();
            assert!(matches!(value, 0..=255u16), "unexpected value returned: {}", value);
        }
        code_length.push(255);
        code_length.shuffle(&mut rng);
        assert_eq!(tree.build_tree(&code_length).unwrap_err(), "too many leaves");

        tree.clear();
        tree.shrink_to_fit();
        for len in 1..=8u8 {
            println!("length: {}", len);
            let nleaves = 1 << len;
            code_length.clear();
            for _ in 1..nleaves {
                code_length.push(len);
                assert_eq!(tree.build_tree(&code_length).unwrap_err(), "missing some leaves");
                assert!(tree.is_empty());
            }
            code_length.push(len);
            tree.build_tree(&code_length).unwrap();
            assert_eq!(tree.tree.len(), nleaves+nleaves-1);
            validate_tree(&tree, nleaves);
            let mut rng = rand::rng();
            let mut rndstream = BitStream::new(RngReader(&mut rng));
            for _ in 0..1_000_000 {
                let value = tree.read_entry(&mut rndstream).unwrap();
                assert!(value < nleaves as u16, "unexpected value returned: {}", value);
            }
            code_length.push(len);
            assert_eq!(tree.build_tree(&code_length).unwrap_err(), "too many leaves");
            assert!(tree.is_empty());
        }
    }

    #[test]
    #[ignore = "long tests"]
    fn hufftree_long_tests() {
        let mut tree = HuffTree::with_capacity(32768);
        let mut rng = rand::rng();
        let vec = &mut Vec::new();
        vec.resize(256, 0);
        for n in 1..=100_000 {
            rng.fill(vec);
            match tree.build_tree(&vec) {
                Ok(()) => println!("random test: {n} OK"),
                Err(_err) => {
                    // println!("random test: {n} ERR: {}", err);
                }
            }
        }

        tree.clear();
        tree.shrink_to_fit();
        println!("length: 255");
        vec.clear();
        loop {
            vec.push(255);
            println!("length: 255 -> ({})", vec.len());
            let err = tree.build_tree(&vec).unwrap_err();
            assert!(tree.is_empty());
            match err {
                "too many code lengths" => break,
                e => assert_eq!(e, "missing some leaves"),
            }
        }

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
