use crate::voxel::Voxel;

/// Per-chunk unique Voxel values. All material index fields in the tree index into this.
/// Bit width of all indices scales with table size: a chunk with 16 unique voxels uses 4 bits.
pub struct MaterialTable {
	pub values: Vec<Voxel>,
}

impl Default for MaterialTable {
	fn default() -> Self {
		Self::new()
	}
}

impl MaterialTable {
	pub fn new() -> Self {
		todo!()
	}

	/// Return the index of an existing entry, or insert and return the new index.
	pub fn get_or_insert(&mut self, voxel: Voxel) -> u32 {
		todo!()
	}

	pub fn get(&self, index: u32) -> Voxel {
		todo!()
	}

	pub fn len(&self) -> u32 {
		todo!()
	}

	pub fn is_empty(&self) -> bool {
		todo!()
	}

	/// ceil(log2(len)) rounded to next power of two, minimum 1.
	pub fn bit_width(&self) -> u8 {
		todo!()
	}
}
