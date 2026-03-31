use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

mod node;
mod voxel;

pub use node::{LEAF_FLAG, child_count, is_leaf_ref, leaf_index, make_leaf_ref};
pub use voxel::{ColorPalette, Voxel};

fn hash_node(occupancy: u64, children: &[u32]) -> u64 {
  let mut h = DefaultHasher::new();
  occupancy.hash(&mut h);
  children.hash(&mut h);
  h.finish()
}

// One interior level of the 64-tree. Uses SoA layout: each field gets its own
// contiguous array indexed by node index.
pub struct Level {
  pub occupancy:      Vec<u64>,
  pub voxel_count:    Vec<u32>,
  pub children_start: Vec<u32>,
  pub children:       Vec<u32>,
  dedup: HashMap<u64, u32>,
}

impl Level {
  pub fn new() -> Self {
    Self {
      occupancy:      Vec::new(),
      voxel_count:    Vec::new(),
      children_start: Vec::new(),
      children:       Vec::new(),
      dedup:          HashMap::new(),
    }
  }

  pub fn len(&self) -> usize {
    self.occupancy.len()
  }

  pub fn insert(&mut self, occupancy: u64, voxel_count: u32, children: &[u32]) -> u32 {
    let hash = hash_node(occupancy, children);

    if let Some(&idx) = self.dedup.get(&hash) {
      return idx;
    }

    let children_start = self.children.len() as u32;
    self.children.extend_from_slice(children);

    let idx = self.occupancy.len() as u32;
    self.occupancy.push(occupancy);
    self.voxel_count.push(voxel_count);
    self.children_start.push(children_start);
    self.dedup.insert(hash, idx);
    idx
  }

  pub fn children_of(&self, node_idx: u32) -> &[u32] {
    let start = self.children_start[node_idx as usize] as usize;
    let count = child_count(self.occupancy[node_idx as usize]) as usize;
    &self.children[start..start + count]
  }
}

pub struct MaterialLut {
  pub data: Vec<Voxel>,
  dedup: HashMap<u64, Vec<u32>>
}

impl MaterialLut {
  pub fn new() -> Self {
    Self {
      data: Vec::new(),
      dedup: HashMap::new(),
    }
  }

  pub fn insert(&mut self, voxel: Voxel) -> u32 {
    let mut h = DefaultHasher::new();
    voxel.hash(&mut h);
    let hash = h.finish();

    if let Some(indices) = self.dedup.get(&hash) {
      for &i in indices {
        if self.data[i as usize] == voxel {
          return i;
        }
      }
    }

    let idx = self.data.len() as u32;
    self.data.push(voxel);
    self.dedup.entry(hash).or_default().push(idx);
    idx
  }

  pub fn mat_index_bits(&self) -> u8 {
    let n = self.data.len();
    if n <= 2 {
      return 1;
    }

    let bits = (usize::BITS - (n - 1).leading_zeros()) as u8;
    bits.next_power_of_two()
  }
}

pub struct Dag {
  pub levels:    Vec<Level>,
  pub materials: MaterialLut,
  pub palette:   ColorPalette,
}

impl Dag {
  pub fn new(depth: usize) -> Self {
    Self {
      levels:    (0..depth).map(|_| Level::new()).collect(),
      materials: MaterialLut::new(),
      palette:   ColorPalette::new(),
    }
  }
}