use crate::tree::{Level, Tree};
use super::{Edit, EditPacket, OrderedEdits, DELETE};

impl<const DEPTH: usize> Tree<DEPTH> {
	pub fn apply_ordered_edits(&mut self, edits: OrderedEdits<DEPTH>) {
		for packet in edits.packets {
			self.apply_edit_packet(packet);
		}
	}

	pub fn apply_edit_packet(&mut self, mut packet: EditPacket<DEPTH>) {
		if packet.paths.is_empty() {
			return;
		}
		packet.sort();

		let values: Vec<u32> = (0..packet.paths.len() as u32)
			.map(|i| packet.lut.get(packet.values.get(i)))
			.collect();

		// Pair each path (as raw 0..63 slots) with its value.
		let edits: Vec<([u8; DEPTH], usize, u32)> = packet.paths.iter()
			.zip(values.iter())
			.map(|(path, &val)| {
				let (raw, level) = path.to_raw();
				(raw, DEPTH - level as usize, val) // depth = number of meaningful slots
			})
			.collect();

		self.apply_edits(&edits);
	}

	fn apply_edits(&mut self, edits: &[([u8; DEPTH], usize, u32)]) {
		// An edit with depth=0 covers the whole tree.
		if let Some(&(_, _, val)) = edits.iter().rfind(|(_, depth, _)| *depth == 0) {
			self.occupied = val != DELETE;
			self.is_leaf = val != DELETE;
			self.value = if val != DELETE { val } else { 0 };
			for lvl in &mut self.levels { *lvl = Level::new(); }
			return;
		}

		self.occupied = true;
		let was_leaf = self.is_leaf;
		let root_value = self.value;
		if self.is_leaf {
			self.is_leaf = false;
			for lvl in &mut self.levels { *lvl = Level::new(); }
		}

		// Build the new tree bottom-up using scratch nodes.
		// scratch[depth] maps node_index -> Node64, built during descent.
		let mut scratch: Vec<Vec<Node64>> = (0..DEPTH).map(|_| Vec::new()).collect();

		// Seed scratch from existing tree.
		for d in 0..DEPTH {
			for n in 0..self.levels[d].node_count() as usize {
				scratch[d].push(Node64::from_level(&self.levels[d], n as u32));
			}
		}
		// Ensure root node exists at depth 0.
		// If the tree was a leaf, expand it: fill all 64 slots with the leaf value.
		if scratch[0].is_empty() {
			let mut root = Node64::default();
			if was_leaf {
				for s in 0..64u8 { root.set_leaf(s, root_value); }
			}
			scratch[0].push(root);
		}

		// Apply edits via recursive descent from the root node.
		descend(&mut scratch, 0, 0, edits);

		// Rebuild all levels bottom-up, producing clean packed arrays and remapping node indices.
		let mut remap: Vec<Vec<u32>> = (0..DEPTH).map(|d| vec![u32::MAX; scratch[d].len()]).collect();

		for d in (0..DEPTH).rev() {
			self.levels[d] = Level::new();
			for (old_idx, node) in scratch[d].iter().enumerate() {
				if node.occupancy == 0 {
					// Empty node — don't emit it; parent will see remap[d][old_idx] = u32::MAX.
					continue;
				}
				// Remap child pointers before emitting.
				let mut remapped = node.clone();
				if d + 1 < DEPTH {
					for slot in 0..64u8 {
						if node.occupied(slot) && !node.is_leaf(slot) {
							let old_child = node.slots[slot as usize].child_node as usize;
							let new_child = remap[d + 1][old_child];
							if new_child == u32::MAX {
								// Child became empty — remove this slot from parent.
								remapped.occupancy &= !(1u64 << slot);
								remapped.leaf_mask &= !(1u64 << slot);
							} else {
								remapped.slots[slot as usize].child_node = new_child;
							}
						}
					}
				}
				if remapped.occupancy == 0 {
					continue; // Node is now empty, don't emit.
				}
				let new_idx = remapped.emit_into(&mut self.levels[d]);
				remap[d][old_idx] = new_idx;
			}
		}

		// Update root state.
		if self.levels[0].node_count() == 0 || self.levels[0].occupancy_mask.first().copied().unwrap_or(0) == 0 {
			self.occupied = false;
		}
	}

	pub fn add_edit(&mut self, edit: Edit<DEPTH>) {
		self.edits.add_edit(edit);
	}
}

// A full 64-slot unpacked node.
#[derive(Clone, Default)]
struct SlotData {
	value: u32,
	child_node: u32,
	occupied: bool,
	leaf: bool,
}

#[derive(Clone)]
struct Node64 {
	slots: [SlotData; 64],
	occupancy: u64,
	leaf_mask: u64,
}

impl Default for Node64 {
	fn default() -> Self {
		Self {
			slots: std::array::from_fn(|_| SlotData::default()),
			occupancy: 0,
			leaf_mask: 0,
		}
	}
}

impl Node64 {
	fn from_level(level: &Level, node: u32) -> Self {
		let mut n = Node64::default();
		let occ = level.occupancy_mask[node as usize];
		let leaf = level.leaf_mask[node as usize];
		let base = level.children_offset[node as usize];
		n.occupancy = occ;
		n.leaf_mask = leaf;
		let mut mask = occ;
		while mask != 0 {
			let slot = mask.trailing_zeros() as u8;
			let rank = (occ & ((1u64 << slot) - 1)).count_ones();
			let idx = base + rank;
			n.slots[slot as usize] = SlotData {
				value: level.values.get(idx),
				child_node: level.node_children.get(idx),
				occupied: true,
				leaf: (leaf >> slot) & 1 != 0,
			};
			mask &= mask - 1;
		}
		n
	}

	fn occupied(&self, slot: u8) -> bool { (self.occupancy >> slot) & 1 != 0 }
	fn is_leaf(&self, slot: u8) -> bool { (self.leaf_mask >> slot) & 1 != 0 }

	fn set_leaf(&mut self, slot: u8, value: u32) {
		let bit = 1u64 << slot;
		if value == DELETE {
			self.occupancy &= !bit;
			self.leaf_mask &= !bit;
			self.slots[slot as usize].occupied = false;
		} else {
			self.occupancy |= bit;
			self.leaf_mask |= bit;
			self.slots[slot as usize] = SlotData { value, child_node: 0, occupied: true, leaf: true };
		}
	}

	fn set_child(&mut self, slot: u8, child_idx: u32, value: u32) {
		let bit = 1u64 << slot;
		self.occupancy |= bit;
		self.leaf_mask &= !bit;
		self.slots[slot as usize] = SlotData { value, child_node: child_idx, occupied: true, leaf: false };
	}

	fn emit_into(&self, level: &mut Level) -> u32 {
		let offset = level.children_len();
		for slot in 0..64u8 {
			if (self.occupancy >> slot) & 1 != 0 {
				let s = &self.slots[slot as usize];
				level.push_child(s.child_node, s.value);
			}
		}
		level.push_node(self.occupancy, self.leaf_mask, offset)
	}
}

// Recursive descent: apply sorted edits to scratch, returning the scratch node index for this subtree.
fn descend<const DEPTH: usize>(
	scratch: &mut Vec<Vec<Node64>>,
	depth: usize,
	node_idx: usize,
	edits: &[([u8; DEPTH], usize, u32)],
) {
	let mut i = 0;
	while i < edits.len() {
		let slot = edits[i].0[depth];
		let j = i + edits[i..].partition_point(|e| e.0[depth] == slot);
		let group = &edits[i..j];

		let terminates_here = group.iter().any(|(_, d, _)| *d == depth + 1);

		if terminates_here {
			// Last terminating edit at this depth wins.
			let &(_, _, val) = group.iter().rfind(|(_, d, _)| *d == depth + 1).unwrap();
			scratch[depth][node_idx].set_leaf(slot, val);
			// If there was a child subtree under this slot, it's now unreachable.
			// The bottom-up rebuild will not emit unreferenced nodes, so nothing extra needed.
		} else if depth + 1 < DEPTH {
			// Need to go deeper. Find or create a child node in scratch.
			let child_idx = {
				let s = &scratch[depth][node_idx];
				if s.occupied(slot) && !s.is_leaf(slot) {
					s.slots[slot as usize].child_node as usize
				} else {
					// Expand: either an existing leaf or an empty slot.
					// If it was a leaf, fill all 64 children with the leaf's value.
					let new_idx = scratch[depth + 1].len();
					let mut child = Node64::default();
					if s.occupied(slot) && s.is_leaf(slot) {
						let leaf_val = s.slots[slot as usize].value;
						for s in 0..64u8 {
							child.set_leaf(s, leaf_val);
						}
					}
					let parent_val = s.slots[slot as usize].value;
					scratch[depth + 1].push(child);
					scratch[depth][node_idx].set_child(slot, new_idx as u32, parent_val);
					new_idx
				}
			};
			descend(scratch, depth + 1, child_idx, group);
		}

		i = j;
	}
}
