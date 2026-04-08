use super::{BitpackedArray, ChildIter, Lut};

// One level of the SVO (sparse voxel 64-tree). SoA layout for GPU traversal.
// Each node covers a 4x4x4 block (64 child slots). Only occupied children are
// stored; child_mask.count_ones() gives the child count for any node.
//
// Children split into two separate bitpacked arrays (ptr_children and
// lut_children) so each can be packed at its natural width without the other
// forcing it wider. leaf_mask marks which occupied children are uniform LUT
// entries vs real node pointers.
pub struct Level {
	pub child_mask: Vec<u64>,         // which of the 64 child slots are occupied
	pub leaf_mask: Vec<u64>,          // which occupied children are uniform (LUT) entries
	pub child_start: Vec<u32>,        // index into ptr_children / lut_children for this node
	pub rep_material: BitpackedArray, // LUT index per node, blended from children (for LOD)
	pub ptr_children: BitpackedArray, // node pointers, bitpacked at ceil(log2(pool_size))
	pub lut_children: BitpackedArray, // uniform LUT indices, bitpacked at ceil(log2(lut_size))
}

impl Level {
	pub fn new() -> Self {
		Self {
			child_mask: Vec::new(),
			leaf_mask: Vec::new(),
			child_start: Vec::new(),
			rep_material: BitpackedArray::new(),
			ptr_children: BitpackedArray::new(),
			lut_children: BitpackedArray::new(),
		}
	}

	pub fn len(&self) -> u32 {
		self.child_mask.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.child_mask.is_empty()
	}

	pub fn ptr_children_of(&self, node_idx: u32) -> ChildIter<'_> {
		let start = self.child_start[node_idx as usize];
		let count = (self.child_mask[node_idx as usize] & !self.leaf_mask[node_idx as usize]).count_ones();
		ChildIter::new(&self.ptr_children, start, start + count)
	}

	pub fn lut_children_of(&self, node_idx: u32) -> ChildIter<'_> {
		let start = self.child_start[node_idx as usize];
		let count = (self.child_mask[node_idx as usize] & self.leaf_mask[node_idx as usize]).count_ones();
		ChildIter::new(&self.lut_children, start, start + count)
	}

	// Appends a node. Returns its index.
	pub fn push(
		&mut self,
		child_mask: u64,
		leaf_mask: u64,
		rep_material: u32,
		ptr_children: &[u32],
		lut_children: &[u32],
	) -> u32 {
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
