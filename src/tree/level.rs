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
		Self {
			occupancy: Vec::new(),
			solid_mask: Vec::new(),
			children_offset: Vec::new(),
			lod_material: BitpackedArray::new(),
			node_children: BitpackedArray::new(),
			leaf_materials: BitpackedArray::new(),
		}
	}

	pub fn node_count(&self) -> u32 {
		self.occupancy.len() as u32
	}

	// --- node reads ---

	pub fn occupancy(&self, node_idx: u32) -> u64 {
		self.occupancy[node_idx as usize]
	}

	pub fn solid_mask(&self, node_idx: u32) -> u64 {
		self.solid_mask[node_idx as usize]
	}

	pub fn children_offset(&self, node_idx: u32) -> u32 {
		self.children_offset[node_idx as usize]
	}

	pub fn lod_material(&self, node_idx: u32) -> u32 {
		self.lod_material.get(node_idx)
	}

	pub fn is_occupied(&self, node_idx: u32, slot: u32) -> bool {
		(self.occupancy[node_idx as usize] >> slot) & 1 != 0
	}

	pub fn is_solid(&self, node_idx: u32, slot: u32) -> bool {
		(self.solid_mask[node_idx as usize] >> slot) & 1 != 0
	}

	pub fn child_idx(&self, node_idx: u32, slot: u32) -> u32 {
		let rank = (self.occupancy[node_idx as usize] & ((1u64 << slot) - 1)).count_ones();
		self.children_offset[node_idx as usize] + rank
	}

	pub fn node_child(&self, idx: u32) -> u32 {
		self.node_children.get(idx)
	}

	pub fn leaf_material(&self, idx: u32) -> u32 {
		self.leaf_materials.get(idx)
	}

	pub fn set_occupancy(&mut self, node_idx: u32, mask: u64) {
		self.occupancy[node_idx as usize] = mask;
	}

	pub fn set_solid_mask(&mut self, node_idx: u32, mask: u64) {
		self.solid_mask[node_idx as usize] = mask;
	}

	pub fn set_lod_material(&mut self, node_idx: u32, value: u32) {
		self.lod_material.set(node_idx, value);
	}

	pub fn set_leaf_material(&mut self, idx: u32, value: u32) {
		self.leaf_materials.set(idx, value);
	}

	pub fn set_node_child(&mut self, idx: u32, value: u32) {
		self.node_children.set(idx, value);
	}
}
