//! Benchmark static Huffman Tree building
#![allow(dead_code)]
use core::fmt;
use rand::{RngExt, Rng, seq::SliceRandom};
use criterion::{
    criterion_group,
    criterion_main,
    Criterion, BenchmarkId, Throughput
};
use delharc::statictree::{*, entry::*};

// Build a flat tree with all the leaves at the bottom.
fn flat_tree_benchmark(c: &mut Criterion) {
    let mut tree = HuffTree::with_capacity(0x4000);
    let mut code_lengths = Vec::with_capacity(0x2000);
    let mut group = c.benchmark_group("Flat");
    group.sample_size(300);
    for depth in [4,5,8,9,10u8] {
    // for depth in 1..=13u8 {
        let nleaves = 1usize << depth;
        code_lengths.clear();
        code_lengths.resize(nleaves, depth);
        tree.build_tree_simple(&code_lengths).unwrap();
        // println!("Flat: {}\n{}", nleaves, DisplayTree(&tree));
        group.throughput(Throughput::Bytes(nleaves as u64));
        group.bench_with_input(BenchmarkId::new("Simple (num leaves)", nleaves), code_lengths.as_slice(), |b, codes| {
            b.iter(|| tree.build_tree_simple(codes).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("Sorted (num leaves)", nleaves), code_lengths.as_slice(), |b, codes| {
            b.iter(|| tree.build_tree_with_sort(codes).unwrap());
        });
    }
    group.finish();
}

// Build a steep tree with each leaf on its own level, except the 2 at the bottom.
fn steep_tree_benchmark(c: &mut Criterion) {
    let mut rng = rand::rng();
    let mut tree = HuffTree::with_capacity(512);
    let mut code_lengths = Vec::with_capacity(256);
    let mut group = c.benchmark_group("Steep");
    group.sample_size(200);
    for &depth in &[/*1u8, 3, 7,*/ 15, 31,/* 63, 127,*/ 255] {
        let nleaves = usize::from(depth) + 1;
        code_lengths.clear();
        code_lengths.extend(1..=depth);
        code_lengths.push(depth);
        assert_eq!(code_lengths.len(), nleaves);
        code_lengths.shuffle(&mut rng);
        tree.build_tree_simple(&code_lengths).unwrap();
        // println!("Steep: {}\n{}", nleaves, DisplayTree(&tree));
        group.throughput(Throughput::Bytes(nleaves as u64));
        group.bench_with_input(BenchmarkId::new("Simple (num leaves)", nleaves), code_lengths.as_slice(), |b, codes| {
            b.iter(|| tree.build_tree_simple(codes).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("Sorted (num leaves)", nleaves), code_lengths.as_slice(), |b, codes| {
            b.iter(|| tree.build_tree_with_sort(codes).unwrap());
        });
    }
    group.finish();
}

// Build a random tree
fn random_tree_benchmark(c: &mut Criterion) {
    let mut rng = rand::rng();
    let mut tree = HuffTree::with_capacity(1024);
    let mut code_lengths = Vec::with_capacity(512);
    let mut group = c.benchmark_group("Random");
    group.sample_size(100);
    let nleaves = 320;
    for i in 1..=10 {
        let max_depth = build_random_lengths(nleaves, u8::MAX, 510, &mut rng, &mut code_lengths);
        assert_eq!(code_lengths.len(), 510);
        tree.build_tree_simple(&code_lengths).unwrap();
        // println!("Random: {}/510\n{}\nMax depth: {}", nleaves, DisplayTree(&tree), max_depth);
        group.throughput(Throughput::Elements(max_depth as u64));
        group.bench_with_input(BenchmarkId::new("Simple ", i), code_lengths.as_slice(), |b, codes| {
            b.iter(|| tree.build_tree_simple(codes).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("Sorted ", i), code_lengths.as_slice(), |b, codes| {
            b.iter(|| tree.build_tree_with_sort(codes).unwrap());
        });
    }
    group.finish();
}

// build a random tree lengths with an upper num of values and max depth
fn build_random_lengths(
        max_values: usize,
        mut max_depth: u8,
        final_size: usize,
        rng: &mut impl Rng,
        out: &mut Vec<u8>
    ) -> u8
{
    out.clear();
    let mut max_leaves = 2usize;
    for level in 1..max_depth {
        let n = out.len();
        let remaining = max_values - n;
        let num_leaves;
        if let Some(margin) = (max_leaves * 2).checked_sub(remaining)  {
            if remaining <= max_leaves {
                max_depth = level;
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
    out.resize(out.len() + max_leaves, max_depth);
    assert_eq!(max_values, out.len(), "max_values: {} != out.len: {}", max_values, out.len());
    assert!(final_size >= out.len(), "final_size: {} < out.len: {}", final_size, out.len());
    out.resize(final_size, 0);
    out.shuffle(rng);
    max_depth
}

struct DisplayTree<'a>(&'a HuffTree);

impl fmt::Display for DisplayTree<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {

        fn fmt_step(
                tree: &[TreeEntry],
                index: usize,
                f: &mut fmt::Formatter<'_>,
                prefix: &mut String,
                count: &mut usize
            ) -> fmt::Result
        {
            let stripped = prefix.trim_start_matches('1');
            let strip_level = (prefix.len() - stripped.len()) / 10 * 10;
            match tree[index].as_node() {
                NodeType::Leaf(code) => {
                    *count += 1;
                    if strip_level >= 10 {
                        writeln!(f, "{:4}: (+{:3}){} -> {}", *count, strip_level, &prefix[strip_level..], code)?;
                    }
                    else {
                        writeln!(f, "{:4}: {} -> {}", *count, prefix, code)?;
                    }
                }
                NodeType::Branch(index) => {
                    prefix.push('0');
                    fmt_step(tree, index as usize, f, prefix, count)?;
                    prefix.pop();
                    prefix.push('1');
                    fmt_step(tree, index as usize + 1, f, prefix, count)?;
                    prefix.pop();
                }
            }
            Ok(())
        }

        if !self.0.is_empty() {
            let mut prefix = String::new();
            fmt_step(&self.0.inspect(), 0, f, &mut prefix, &mut 0)?;
        }
        Ok(())
    }
}

criterion_group!(benches,
    flat_tree_benchmark,
    steep_tree_benchmark,
    random_tree_benchmark);

criterion_main!(benches);
