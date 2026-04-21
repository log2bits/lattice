use crate::bitpacked::BitpackedArray;

#[derive(Default)]
pub struct Level {
	pub occupancy_mask: Vec<u64>,
	pub terminal_mask: Vec<u64>,
	pub children_offset: Vec<u32>,
	pub node_children: BitpackedArray,
	pub materials: BitpackedArray,
}

impl Level {
	pub fn with_root_node() -> Self {
		Self {
			occupancy_mask: vec![0],
			terminal_mask: vec![0],
			children_offset: vec![0],
			node_children: BitpackedArray::new(),
			materials: BitpackedArray::new(),
		}
	}

	pub fn clear(&mut self) {
		self.occupancy_mask.clear();
		self.terminal_mask.clear();
		self.children_offset.clear();
		self.node_children.clear();
		self.materials.clear();
	}

	pub fn node_count(&self) -> u32 {
		self.occupancy_mask.len() as u32
	}

	pub fn is_occupied(&self, node_idx: u32, slot: u32) -> bool {
		(self.occupancy_mask[node_idx as usize] >> slot) & 1 != 0
	}

	pub fn is_terminal(&self, node_idx: u32, slot: u32) -> bool {
		(self.terminal_mask[node_idx as usize] >> slot) & 1 != 0
	}

	/// Packed index into `node_children` and `materials` for the child at `slot`.
	pub fn child_idx(&self, node_idx: u32, slot: u32) -> u32 {
		let rank = (self.occupancy_mask[node_idx as usize] & ((1u64 << slot) - 1)).count_ones();
		self.children_offset[node_idx as usize] + rank
	}

	/// Push a non-terminal child entry (non-leaf levels only).
	pub fn push_child(&mut self, child_node_idx: u32, lod_material: u32) {
		self.node_children.push(child_node_idx);
		self.materials.push(lod_material);
	}

	/// Push a terminal child entry (non-leaf levels only). Caller sets terminal_mask bit.
	pub fn push_terminal(&mut self, material: u32) {
		self.node_children.push(0);
		self.materials.push(material);
	}

	/// Push a leaf-level material entry. node_children stays empty at the leaf level.
	pub fn push_leaf_material(&mut self, material: u32) {
		self.materials.push(material);
	}
}
