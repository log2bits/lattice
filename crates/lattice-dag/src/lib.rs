mod node;
pub use node::{Node, LEAF_FLAG, is_leaf_ref, leaf_index, make_leaf_ref};

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash_node(occupancy: u64, children: &[u32]) -> u64 {
  let mut h = DefaultHasher::new();
  occupancy.hash(&mut h);
  children.hash(&mut h);
  h.finish()
}

// One interior level of the 64-tree. Deduplication is always on. Every insert
// checks the content-addressed table and returns an existing index if the node
// already exists. This has no runtime cost -- the HashMap is only used during
// construction in lattice-pack and is not part of the runtime structure uploaded
// to VRAM.
//
// Nodes are inserted in the order the construction code visits them. Building
// breadth-first gives spatially adjacent nodes adjacent pool indices, which
// helps cache behavior during GPU traversal.
pub struct Level {
  pub nodes: Vec<Node>,
  pub children: Vec<u32>,
  dedup: HashMap<u64, u32>,
}

impl Level {
  pub fn new() -> Self {
    Self {
      nodes: Vec::new(),
      children: Vec::new(),
      dedup: HashMap::new(),
    }
  }

  // Inserts an interior node and returns its pool index. If an identical node
  // already exists, returns the existing index without inserting a duplicate.
  // `children` must already have LEAF_FLAG set on any entries that point to leaves.
  pub fn insert(&mut self, occupancy: u64, children: &[u32]) -> u32 {
    let hash = hash_node(occupancy, children);

    if let Some(&idx) = self.dedup.get(&hash) {
      return idx;
    }

    let children_start = self.children.len() as u32;
    self.children.extend_from_slice(children);
    let idx = self.nodes.len() as u32;
    self.nodes.push(Node { occupancy, children_start });
    self.dedup.insert(hash, idx);
    idx
  }

  pub fn children_of(&self, node: &Node) -> &[u32] {
    let start = node.children_start as usize;
    let end = start + node.child_count() as usize;
    &self.children[start..end]
  }
}

// The leaf pool. Each entry is one voxel's data. Deduplication is always on.
// For geometry (T = ()), the pool ends up with exactly one entry. For color
// (T = some material type), the pool has one entry per unique material in the scene.
pub struct LeafPool<T> {
  pub data: Vec<T>,
  dedup: HashMap<u64, u32>,
}

impl<T: Hash> LeafPool<T> {
  pub fn new() -> Self {
    Self {
      data: Vec::new(),
      dedup: HashMap::new(),
    }
  }

  // Inserts a voxel's data and returns its pool index. Returns an existing index
  // if identical data was already inserted. Wrap the result with make_leaf_ref()
  // before storing it in a Level's children array.
  pub fn insert(&mut self, leaf: T) -> u32 {
    let mut h = DefaultHasher::new();
    leaf.hash(&mut h);
    let hash = h.finish();

    if let Some(&idx) = self.dedup.get(&hash) {
      return idx;
    }

    let idx = self.data.len() as u32;
    self.data.push(leaf);
    self.dedup.insert(hash, idx);
    idx
  }
}

// The world-scale DAG arena. One instance for geometry (T = ()), one for color
// (T = some material type). Spans the entire world. A spatial region is just a
// u32 root index into levels[0].
//
// Deduplication is always on at every level. There is no per-level flag. The
// runtime layout is identical whether a node was deduplicated or not -- the HashMap
// only lives during construction and never reaches VRAM. Dedup always reduces or
// maintains pool size, never makes it worse, and at the levels where it matters
// most (near the leaves) it cuts the pool dramatically.
//
// Tree depth is set at construction time. A tree with N interior levels covers a
// (4^N)^3 voxel region down to individual voxels at the leaves.
pub struct Dag<T> {
  pub levels: Vec<Level>,
  pub leaves: LeafPool<T>,
}

impl<T: Hash> Dag<T> {
  // Creates an empty world-scale DAG with `depth` interior levels.
  // A depth of 3 covers a 64^3 voxel region (4^3 per level).
  pub fn new(depth: usize) -> Self {
    Self {
      levels: (0..depth).map(|_| Level::new()).collect(),
      leaves: LeafPool::new(),
    }
  }
}
