use super::{BitpackedArray, ChildIter, Lut};

// One level of the SVO (sparse voxel 64-tree). SoA layout for GPU traversal.
// Each node covers a 4x4x4 block (64 child slots). Only occupied children are
// stored; child_mask.count_ones() gives the child count for any node.
pub struct Level {
	pub child_mask: Vec<u64>,       // 64-bit occupancy, one per node
	pub child_start: Vec<u32>,      // index into this level's children array
	pub rep_material: Vec<u32>,     // per-chunk LUT index, blended from children (for LOD)
	pub children: BitpackedArray,   // packed child entries (LEAF_FLAG|lut_idx or node_ptr)
}

impl Level {
	pub fn new() -> Self {
		Self {
			child_mask: Vec::new(),
			child_start: Vec::new(),
			rep_material: Vec::new(),
			children: BitpackedArray::new(),
		}
	}

	pub fn len(&self) -> u32 {
		self.child_mask.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.child_mask.is_empty()
	}

	pub fn children_of(&self, node_idx: u32) -> ChildIter<'_> {
		let start = self.child_start[node_idx as usize];
		let count = self.child_mask[node_idx as usize].count_ones();
		ChildIter::new(&self.children, start, start + count)
	}

	// Appends a node. Returns its index.
	pub fn push(&mut self, child_mask: u64, rep_material: u32, children: &[u32]) -> u32 {
		todo!()
	}
}

impl Default for Level {
	fn default() -> Self {
		Self::new()
	}
}

// Per-chunk data: the root node index into levels[0] and the material palette.
// Two grid cells pointing to the same Chunk index share all tree and material data.
pub struct Chunk {
	pub root_node_index: u32,
	pub lut: Lut,
}

impl Chunk {
	pub fn new(root_node_index: u32) -> Self {
		Self {
			root_node_index,
			lut: Lut::new(),
		}
	}
}
