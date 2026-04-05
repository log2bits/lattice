use std::collections::HashMap;

use super::{BitpackedArray, ChildIter};

// One level of a Geometry DAG section. Deduplicates on occupancy only -- two
// nodes with the same shape but different materials underneath share a node.
// Material data is tracked separately via the Dolonius running offset.
pub struct GeometryDagLevel {
	pub occupancy: Vec<u64>,
	pub voxel_count: Vec<u32>,
	pub children_start: Vec<u32>,
	pub children: BitpackedArray,
	pub(crate) lookup: HashMap<BitpackedArray, u32>, // build-time only
}

impl GeometryDagLevel {
	pub fn new() -> Self {
		Self {
			occupancy: Vec::new(),
			voxel_count: Vec::new(),
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

	// Deduplicates on occupancy only. The children slice is stored but not used
	// as part of the hash key -- the same geometry node is shared regardless of
	// what materials sit underneath it.
	pub fn insert(&mut self, occupancy: u64, voxel_count: u32, children: &[u32]) -> u32 {
		todo!()
	}
}

impl Default for GeometryDagLevel {
	fn default() -> Self {
		Self::new()
	}
}

// Per-root data for a Geometry DAG section. Owns the local voxel LUT and the
// Dolonius materials array for this root's subtree.
//
// leaf_start is a logical entry index into the bottom level's children
// BitpackedArray. The on-disk format stores a byte offset; the conversion
// happens at serialization time.
pub struct GeometryDagRoot {
	pub root_node_index: u32,
	pub lut_index_bits: u8,
	pub lut_entries: Vec<u32>,
	pub leaf_start: u32,
	pub materials: BitpackedArray,
}
