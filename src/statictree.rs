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
use std::io;
use crate::bitstream::BitRead;

/// A static Huffman tree.
#[derive(Debug, Clone)]
pub struct HuffTree {
    tree: Vec<TreeEntry>
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct TreeEntry(u16);

const LEAF_BIT: u16 = 1u16.rotate_right(1);

enum NodeType {
    Leaf(u16),
    Branch(u16),
}

impl TreeEntry {
    const MAX_VALUE: usize = LEAF_BIT as usize - 1;

    #[inline]
    fn leaf(value: u16) -> Self {
        TreeEntry(value | LEAF_BIT)
    }

    #[inline]
    fn branch(index: usize) -> Result<Self, &'static str> {
        if index > Self::MAX_VALUE {
            return Err("too many tree items");
        }
        Ok(TreeEntry(index as u16))
    }

    #[inline]
    fn as_type(self) -> NodeType {
        let TreeEntry(value) = self;
        if value & LEAF_BIT == LEAF_BIT {
            NodeType::Leaf(value & !LEAF_BIT)
        }
        else {
            NodeType::Branch(value)
        }
    }
}

impl HuffTree {
    /// Creates a new and empty `HuffTree`.
    ///
    /// Any attempt to read from a new tree will result in panic.
    pub fn new() -> HuffTree {
        let tree = Vec::new();
        HuffTree { tree }
    }
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
    /// * Missing leaves are being populated with the `value` of `0`.
    /// * If too many entries contain the same `length`, exceeding the given `length` capacity, an error
    ///   is being returned.
    /// * If the size of the argument slice is larger than or equal to the [core::u16::MAX], an error is
    ///   being returned.
    /// * If the number of created nodes would exceed [core::u16::MAX], an error is being returned.
    pub fn build_tree(&mut self, value_lengths: &[u8]) -> Result<(), &'static str> {
        // println!("({}) {:?}", value_lengths.len(), value_lengths);
        if value_lengths.len() > TreeEntry::MAX_VALUE {
            return Err("too many code lengths");
        }
        let max_values: u16 = value_lengths.len() as u16;
        let tree = &mut self.tree;
        tree.clear();
        let mut max_allocated: usize = 1;
        for value_len in 1u8..=u8::max_value() {
            // add new length nodes
            for _ in  tree.len()..max_allocated {
                match TreeEntry::branch(max_allocated) {
                    Ok(branch) => tree.push(branch),
                    Err(e) => {
                        tree.clear(); // make sure no outstanding branch index exists
                        return Err(e);
                    }
                }
                max_allocated += 2;
            }
            // fill new length with leaves
            let more = value_lengths.iter().copied().zip(0..max_values)
                                   .fold(false, |mut more, (len, value)| {
                if len == value_len {
                    tree.push(TreeEntry::leaf(value));
                }
                else if len > value_len {
                    more = true;
                }
                more
            });
            if tree.len() > max_allocated {
                return Err("too many leaves");
            }
            if !more {
                break;
            }
        }
        // println!("tree missing leaves: {}", max_allocated - tree.len());
        tree.extend(
            (tree.len()..max_allocated).map(|_| TreeEntry::leaf(0) )
        );
        Ok(())
    }
    /// Returns the `value` of the leaf by following the path read from the given bit reader.
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
    pub fn read_entry<R: BitRead>(&self, mut entry: R) -> io::Result<u16> {
        let tree = &self.tree;
        let mut node = &tree[0]; // panics if tree uninitialized
        loop {
            match node.as_type() {
                NodeType::Leaf(code) => return Ok(code),
                NodeType::Branch(index) => {
                    let index = index as usize + entry.read_bits::<usize>(1)?;
                    node = unsafe { tree.get_unchecked(index) };
                    // safe because tree was initialized in a sane way
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

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem;
    use crate::bitstream::BitStream;
    #[test]
    fn hufftree_works() {
        assert_eq!(mem::size_of::<TreeEntry>(), 2);
        assert_eq!(LEAF_BIT, 0x8000);
        let mut tree = HuffTree::new();
        println!("{:?}", tree);
        println!("{}", tree);
        tree.set_single(42);
        let path = BitStream::new([].as_ref());
        assert_eq!(tree.read_entry(path).unwrap(), 42);
        println!("{:?}", tree);
        println!("{}", tree);
        tree.build_tree(&[0, 0, 0, 1, 0, 3, 3, 0, 4, 4, 5, 0, 0, 5, 5, 5]).unwrap();
        println!("{:?}", tree);
        println!("{}len: {}", tree, tree.tree.len());
        let bits: &[u8] = &[0b01001011, 0b10011011, 0b11001110, 0b11111011, 0b11100000];
        let mut path = BitStream::new(bits);
        let mut res = Vec::new();
        for _ in 0..9 {
            res.push(tree.read_entry(path.by_ref()).unwrap());
        }
        assert_eq!(res, [3, 5, 6, 8, 9, 10, 13, 14, 15]);
    }
}
