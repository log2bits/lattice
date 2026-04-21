use super::Chunk;
use crate::{bitpacked::BitpackedArray, voxel::Voxel};

pub struct Edit {
	pub pos: [u32; 3],
	pub level: u8, // tree level; depth-1 = single voxel, lower = larger cube
	pub fill: Option<Voxel>,
}

impl Chunk {
	/// Apply all queued edit packets to the tree without canonicalizing.
	/// Returns true if anything was applied. The tree remains structurally valid but
	/// may contain orphaned nodes from previous builds. Call `flush_edits` for a
	/// fully deduplicated SVDAG.
	pub fn flush_pending_edits(&mut self) -> bool {
		if self.pending.is_empty() {
			return false;
		}
		let packets = std::mem::take(&mut self.pending);
		let depth = self.depth();

		let total: u32 = packets.iter().map(|p| p.edits.len() as u32).sum();
		let mat_bits = BitpackedArray::min_bits((self.materials.len() + total).max(1));
		for level in &mut self.levels {
			level.materials.repack_in_place(mat_bits);
			level
				.node_children
				.repack_in_place(BitpackedArray::min_bits(level.node_count().max(1) + total));
		}

		for mut packet in packets {
			if !packet.presorted {
				radsort::sort_by_key(&mut packet.edits, |e: &Edit| {
					Self::tree_order_key(e.pos, depth)
				});
				packet.edits.dedup_by(|later, earlier| {
					later.pos == earlier.pos && later.level == earlier.level && {
						*earlier = std::mem::replace(
							later,
							Edit {
								pos: [0, 0, 0],
								level: 0,
								fill: None,
							},
						);
						true
					}
				});
			}
			if !packet.edits.is_empty() {
				self.root = self.rebuild_subtree(0, self.root, &packet.edits);
			}
		}

		self.svdag_clean = false;
		true
	}

	/// Apply all queued edits and canonicalize the SVDAG.
	/// Returns true if any work was done (edits applied or tree deduplicated).
	pub fn flush_edits(&mut self) -> bool {
		let applied = self.flush_pending_edits();
		let was_dirty = !self.svdag_clean;
		if was_dirty {
			self.canonicalize();
			self.svdag_clean = true;
		}
		applied || was_dirty
	}

	fn rebuild_subtree(&mut self, level_idx: u8, node_idx: u32, edits: &[Edit]) -> u32 {
		if edits.is_empty() {
			return node_idx;
		}

		let depth = self.depth();
		let is_leaf = level_idx == depth - 1;

		// Single linear scan to partition edits into per-slot ranges.
		// Edits are in tree-order so each slot's edits are contiguous.
		let mut slot_ends = [0u32; 64];
		let mut i = 0u32;
		for slot in 0..64u32 {
			while (i as usize) < edits.len()
				&& Self::slot_at_level(edits[i as usize].pos, level_idx as u32, depth) == slot
			{
				i += 1;
			}
			slot_ends[slot as usize] = i;
		}

		let occ = self.levels[level_idx as usize].occupancy_mask[node_idx as usize];
		let term = self.levels[level_idx as usize].terminal_mask[node_idx as usize];

		// Preload current slot data before any mutations to the level arrays.
		let mut slot_materials = [0u32; 64];
		let mut slot_children = [0u32; 64];
		for slot in 0..64u32 {
			if (occ >> slot) & 1 != 0 {
				let ci = self.levels[level_idx as usize].child_idx(node_idx, slot);
				slot_materials[slot as usize] = self.levels[level_idx as usize].materials.get(ci);
				slot_children[slot as usize] = if !is_leaf && (term >> slot) & 1 == 0 {
					self.levels[level_idx as usize].node_children.get(ci)
				} else {
					0
				};
			}
		}

		let new_node_idx = self.levels[level_idx as usize].occupancy_mask.len() as u32;
		let new_offset = if is_leaf {
			self.levels[level_idx as usize].materials.len()
		} else {
			self.levels[level_idx as usize].node_children.len()
		};
		let mut new_occ = 0u64;
		let mut new_term = 0u64;

		let mut slot_start = 0usize;
		for slot in 0..64u32 {
			let slot_end = slot_ends[slot as usize] as usize;
			let slot_edits = &edits[slot_start..slot_end];
			slot_start = slot_end;

			let occupied = (occ >> slot) & 1 != 0;
			let terminal = (term >> slot) & 1 != 0;
			let mat = slot_materials[slot as usize];
			let child_node = slot_children[slot as usize];

			if slot_edits.is_empty() {
				if !occupied {
					continue;
				}
				if is_leaf || terminal {
					if is_leaf {
						self.levels[level_idx as usize].push_leaf_material(mat);
					} else {
						self.levels[level_idx as usize].push_terminal(mat);
					}
					new_term |= 1u64 << slot;
				} else {
					self.levels[level_idx as usize].push_child(child_node, mat);
				}
				new_occ |= 1u64 << slot;
				continue;
			}

			// An edit targeting exactly this level fills/clears the whole subtree.
			let terminal_edit = slot_edits.iter().rev().find(|e| e.level == level_idx);
			if let Some(te) = terminal_edit {
				if let Some(voxel) = te.fill {
					let m = self.materials.intern(voxel);
					if is_leaf {
						self.levels[level_idx as usize].push_leaf_material(m);
					} else {
						self.levels[level_idx as usize].push_terminal(m);
					}
					new_occ |= 1u64 << slot;
					new_term |= 1u64 << slot;
				}
			} else {
				let child_for_descent = if !occupied {
					self.alloc_empty_node(level_idx + 1)
				} else if terminal {
					self.expand_terminal(level_idx + 1, mat)
				} else {
					child_node
				};

				let new_child = self.rebuild_subtree(level_idx + 1, child_for_descent, slot_edits);

				if self.levels[(level_idx + 1) as usize].occupancy_mask[new_child as usize] == 0 {
					continue;
				}
				if let Some(uniform_mat) = self.uniform_terminal_material(level_idx + 1, new_child)
				{
					self.levels[level_idx as usize].push_terminal(uniform_mat);
					new_term |= 1u64 << slot;
				} else {
					let lod = self.node_lod(level_idx + 1, new_child);
					self.levels[level_idx as usize].push_child(new_child, lod);
				}
				new_occ |= 1u64 << slot;
			}
		}

		self.levels[level_idx as usize].occupancy_mask.push(new_occ);
		self.levels[level_idx as usize].terminal_mask.push(new_term);
		self.levels[level_idx as usize]
			.children_offset
			.push(new_offset);
		new_node_idx
	}
}
