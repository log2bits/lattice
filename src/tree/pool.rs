use crate::bitpacked::BitpackedArray;

/// SoA storage for one depth level of the 64-tree.
/// Every array is parallel: index i refers to node i at this depth.
pub struct NodePool {
	/// Which of 64 child slots are occupied.
	pub occupancy: Vec<u64>,
	/// Which occupied children are uniform (entire subtree is one material, stop traversing).
	pub solid_mask: Vec<u64>,
	/// Where this node's children begin in the next depth's NodePool.
	pub children_offset: Vec<u32>,
	/// Blended material index per node, used when LOD cuts traversal short.
	pub lod_material: BitpackedArray,
	/// Indices into the next depth's NodePool, bitpacked at ceil(log2(pool_size)) bits.
	pub node_children: BitpackedArray,
	/// MaterialTable indices for solid subtree children, bitpacked at ceil(log2(table_size)) bits.
	pub leaf_materials: BitpackedArray,
}

impl NodePool {
	pub fn new() -> Self {
		todo!()
	}

	pub fn len(&self) -> usize {
		self.occupancy.len()
	}

	pub fn is_empty(&self) -> bool {
		self.occupancy.is_empty()
	}
}

impl Default for NodePool {
	fn default() -> Self {
		Self::new()
	}
}
