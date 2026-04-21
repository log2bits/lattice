use super::Chunk;
use crate::tree::Level;
use rustc_hash::FxHashMap;

struct NodeSnapshot {
	occ: u64,
	term: u64,
	// One entry per occupied slot in rank order: (is_terminal, remapped_child_or_0, material).
	children: Vec<(bool, u32, u32)>,
	// Canonical comparison key: [occ_lo, occ_hi, term_lo, term_hi, child0, mat0, ...]
	// child is 0 for terminal slots; term encodes which slots are terminal,
	// so two nodes with the same sig are structurally identical.
	sig: Vec<u32>,
}

fn snapshot_reachable_nodes(
	level: &Level,
	reachable: &[u32],
	child_remap: &[u32],
	is_leaf: bool,
) -> Vec<NodeSnapshot> {
	let mut nodes = Vec::with_capacity(reachable.len());

	for &n in reachable {
		let n = n as usize;
		let occ = level.occupancy_mask[n];
		let term = level.terminal_mask[n];
		let offset = level.children_offset[n];
		let count = occ.count_ones() as usize;

		let mut children = Vec::with_capacity(count);
		let mut rank = 0u32;
		for slot in 0..64u64 {
			if (occ >> slot) & 1 == 0 {
				continue;
			}
			let mat = level.materials.get(offset + rank);
			let is_slot_terminal = is_leaf || (term >> slot) & 1 != 0;
			let child = if is_slot_terminal {
				0
			} else {
				child_remap[level.node_children.get(offset + rank) as usize]
			};
			children.push((is_slot_terminal, child, mat));
			rank += 1;
		}

		let mut sig = Vec::with_capacity(4 + count * 2);
		sig.push(occ as u32);
		sig.push((occ >> 32) as u32);
		sig.push(term as u32);
		sig.push((term >> 32) as u32);
		for &(_, c, m) in &children {
			sig.push(c);
			sig.push(m);
		}

		nodes.push(NodeSnapshot {
			occ,
			term,
			children,
			sig,
		});
	}

	nodes
}

impl Chunk {
	/// Deduplicates identical nodes within each level bottom-up.
	/// Unreachable orphaned nodes (left by partial rebuilds) are dropped.
	pub fn canonicalize(&mut self) {
		let depth = self.depth() as usize;
		let reachable = self.collect_reachable();
		let mut remaps: Vec<Vec<u32>> = vec![Vec::new(); depth];

		for level_idx in (0..depth).rev() {
			let is_leaf = level_idx == depth - 1;
			let child_remap = if level_idx + 1 < depth {
				&remaps[level_idx + 1]
			} else {
				&[] as &[u32]
			};
			let nodes = snapshot_reachable_nodes(
				&self.levels[level_idx],
				&reachable[level_idx],
				child_remap,
				is_leaf,
			);

			let mut sig_to_new: FxHashMap<Vec<u32>, u32> = FxHashMap::default();
			let old_count = self.levels[level_idx].occupancy_mask.len();
			let mut remap = vec![0u32; old_count];

			self.levels[level_idx].clear();

			for (i, node) in nodes.iter().enumerate() {
				let old_idx = reachable[level_idx][i];
				let new_idx = if let Some(&existing) = sig_to_new.get(&node.sig) {
					existing
				} else {
					let idx = sig_to_new.len() as u32;
					let offset = if is_leaf {
						self.levels[level_idx].materials.len()
					} else {
						self.levels[level_idx].node_children.len()
					};
					for &(is_terminal, child, mat) in &node.children {
						if is_leaf || is_terminal {
							if is_leaf {
								self.levels[level_idx].push_leaf_material(mat);
							} else {
								self.levels[level_idx].push_terminal(mat);
							}
						} else {
							self.levels[level_idx].push_child(child, mat);
						}
					}
					self.levels[level_idx].occupancy_mask.push(node.occ);
					self.levels[level_idx].terminal_mask.push(node.term);
					self.levels[level_idx].children_offset.push(offset);
					sig_to_new.insert(node.sig.clone(), idx);
					idx
				};
				remap[old_idx as usize] = new_idx;
			}

			remaps[level_idx] = remap;
		}

		self.root = remaps[0][self.root as usize];
	}

	/// DFS from self.root collecting reachable node indices per level.
	fn collect_reachable(&self) -> Vec<Vec<u32>> {
		let depth = self.depth() as usize;
		let mut reachable: Vec<Vec<u32>> = vec![Vec::new(); depth];
		reachable[0].push(self.root);

		for level_idx in 0..depth - 1 {
			let level = &self.levels[level_idx];
			let mut next: Vec<u32> = Vec::new();
			for &node_idx in &reachable[level_idx] {
				let occ = level.occupancy_mask[node_idx as usize];
				let term = level.terminal_mask[node_idx as usize];
				let offset = level.children_offset[node_idx as usize];
				let mut rank = 0u32;
				for slot in 0..64u32 {
					if (occ >> slot) & 1 == 0 {
						continue;
					}
					if (term >> slot) & 1 == 0 {
						next.push(level.node_children.get(offset + rank));
					}
					rank += 1;
				}
			}
			next.sort_unstable();
			next.dedup();
			reachable[level_idx + 1] = next;
		}

		reachable
	}
}
