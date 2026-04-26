use crate::types::BitpackedArray;

#[derive(Clone)]
pub struct Level {
	pub occupancy_mask: Vec<u64>,
	pub leaf_mask: Vec<u64>,
	pub children_offset: Vec<u32>,
	pub node_children: BitpackedArray,
	pub values: BitpackedArray,
}

impl Level {
	pub fn new() -> Self {
		Self {
			occupancy_mask: Vec::new(),
			leaf_mask: Vec::new(),
			children_offset: Vec::new(),
			node_children: BitpackedArray::new(),
			values: BitpackedArray::new(),
		}
	}

	// --- Queries ---

	pub fn node_count(&self) -> u32 {
		self.occupancy_mask.len() as u32
	}

	pub fn bytes(&self) -> usize {
		self.occupancy_mask.len() * 8
			+ self.leaf_mask.len() * 8
			+ self.children_offset.len() * 4
			+ self.node_children.bytes()
			+ self.values.bytes()
	}

	// Number of occupied leaf slots across all nodes at this level.
	pub fn leaf_count(&self) -> u64 {
		self.occupancy_mask.iter().zip(self.leaf_mask.iter())
			.map(|(&occ, &leaf)| (occ & leaf).count_ones() as u64)
			.sum()
	}

	pub fn child_count(&self, node: u32) -> u32 {
		self.occupancy_mask[node as usize].count_ones()
	}

	pub fn is_occupied(&self, node: u32, slot: u8) -> bool {
		debug_assert!(node < self.node_count());
		debug_assert!(slot < 64);
		(self.occupancy_mask[node as usize] >> slot) & 1 != 0
	}

	pub fn is_leaf(&self, node: u32, slot: u8) -> bool {
		debug_assert!(node < self.node_count());
		debug_assert!(slot < 64);
		(self.leaf_mask[node as usize] >> slot) & 1 != 0
	}

	// Packed index into node_children and values for the child at slot.
	pub fn child_idx(&self, node: u32, slot: u8) -> u32 {
		debug_assert!(node < self.node_count());
		debug_assert!(self.is_occupied(node, slot), "slot {slot} is not occupied");
		let rank = (self.occupancy_mask[node as usize] & ((1u64 << slot) - 1)).count_ones();
		self.children_offset[node as usize] + rank
	}

	// Child node index for a non-leaf slot.
	pub fn get_child(&self, node: u32, slot: u8) -> u32 {
		debug_assert!(
			!self.is_leaf(node, slot),
			"slot {slot} is a leaf, has no child node"
		);
		self.node_children.get(self.child_idx(node, slot))
	}

	// Value for any occupied slot (leaf value or LOD representative).
	pub fn get_value(&self, node: u32, slot: u8) -> u32 {
		self.values.get(self.child_idx(node, slot))
	}

	// Current packed array length; record this as children_offset before pushing children.
	pub fn children_len(&self) -> u32 {
		self.node_children.len()
	}

	// Append a node. Call children_len() for the children_offset, then push_child()
	// for each occupied slot before calling this.
	pub fn push_node(&mut self, occupancy_mask: u64, leaf_mask: u64, children_offset: u32) -> u32 {
		debug_assert_eq!(
			children_offset + occupancy_mask.count_ones(),
			self.children_len(),
			"push_child must be called for each occupied slot before push_node"
		);
		let node_idx = self.node_count();
		self.occupancy_mask.push(occupancy_mask);
		self.leaf_mask.push(leaf_mask);
		self.children_offset.push(children_offset);
		node_idx
	}

	// Edit a node's masks and offset in place. Does not touch node_children or values.
	pub fn set_node(
		&mut self,
		node_idx: u32,
		occupancy_mask: u64,
		leaf_mask: u64,
		children_offset: u32,
	) {
		debug_assert!(node_idx < self.node_count());
		let i = node_idx as usize;
		self.occupancy_mask[i] = occupancy_mask;
		self.leaf_mask[i] = leaf_mask;
		self.children_offset[i] = children_offset;
	}

	// Mark all occupied slots of a node as leaves. Does not update values.
	pub fn set_leaf(&mut self, node: u32) {
		debug_assert!(node < self.node_count());
		self.leaf_mask[node as usize] = self.occupancy_mask[node as usize];
	}

	pub fn set_slot_leaf(&mut self, node: u32, slot: u8) {
		debug_assert!(node < self.node_count());
		debug_assert!(slot < 64);
		self.leaf_mask[node as usize] |= 1u64 << slot;
	}

	pub fn clear_slot_leaf(&mut self, node: u32, slot: u8) {
		debug_assert!(node < self.node_count());
		debug_assert!(slot < 64);
		self.leaf_mask[node as usize] &= !(1u64 << slot);
	}

	// Append one (child_node, value) pair to the packed arrays.
	// Leaf slots use child_node = 0; non-leaf slots use the child node index.
	pub fn push_child(&mut self, child_node: u32, value: u32) {
		self.node_children.push(child_node);
		self.values.push(value);
	}

	// Update a slot's value in place.
	pub fn set_value(&mut self, node: u32, slot: u8, value: u32) {
		debug_assert!(self.is_occupied(node, slot));
		let idx = self.child_idx(node, slot);
		self.values.set(idx, value);
	}

	// Convenience: push a node where every occupied slot is a leaf with the same value.
	pub fn push_leaf(&mut self, occupancy_mask: u64, value: u32) -> u32 {
		let children_offset = self.children_len();
		for _ in 0..occupancy_mask.count_ones() {
			self.push_child(0, value);
		}
		self.push_node(occupancy_mask, occupancy_mask, children_offset)
	}
}
