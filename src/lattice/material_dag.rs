use std::collections::HashMap;

use super::{BitpackedArray, ChildIter};

// One level of a Material DAG section. Deduplicates on both occupancy and
// children -- two nodes only share a node if their shape and all leaf data
// match exactly. Material data is inline in the leaf entries, not a separate
// array.
pub struct MaterialDagLevel {
	pub occupancy: Vec<u64>,
	pub children_start: Vec<u32>,
	pub children: BitpackedArray,
	pub(crate) lookup: HashMap<BitpackedArray, u32>, // build-time only
}

impl MaterialDagLevel {
	pub fn new() -> Self {
		Self {
			occupancy: Vec::new(),
			children_start: Vec::new(),
			children: BitpackedArray::new(),
			lookup: HashMap::new(),
		}
	}

	pub fn len(&self) -> u32 {
		self.occupancy.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.occupancy.is_empty()
	}

	pub fn children_of(&self, node_idx: u32) -> ChildIter<'_> {
		let start = self.children_start[node_idx as usize];
		let count = self.occupancy[node_idx as usize].count_ones();
		ChildIter::new(&self.children, start, start + count)
	}

	// Deduplicates on occupancy and children together. Both the shape and the
	// leaf values must match for two nodes to share a DAG node.
	pub fn insert(&mut self, occupancy: u64, children: &[u32]) -> u32 {
		todo!()
	}
}

impl Default for MaterialDagLevel {
	fn default() -> Self {
		Self::new()
	}
}

// Per-root data for a Material DAG section. Owns the local voxel LUT for this
// root's subtree. No materials array -- material data is inline in leaf entries.
//
// leaf_start is a logical entry index into the bottom level's children
// BitpackedArray. The on-disk format stores a byte offset; the conversion
// happens at serialization time.
pub struct MaterialDagRoot {
	pub root_node_index: u32,
	pub lut_index_bits: u8,
	pub lut_entries: Vec<u32>,
	pub leaf_start: u32,
}
