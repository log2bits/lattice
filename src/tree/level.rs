use crate::bitpacked::BitpackedArray;
pub struct Level {
	pub occupancy: Vec<u64>,
	pub solid_mask: Vec<u64>,
	pub children_offset: Vec<u32>,
	pub lod_material: BitpackedArray,
	pub node_children: BitpackedArray,
	pub leaf_materials: BitpackedArray,
}

impl Level {
	pub fn new() -> Self {
		todo!()
	}
}
