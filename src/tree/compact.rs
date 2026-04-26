use std::collections::HashMap;
use crate::tree::{Level, Tree};

impl<const DEPTH: usize> Tree<DEPTH> {
	// Deduplicates identical subtrees and drops orphaned nodes.
	// Safe to call at any time; required for accurate bytes()/leaf_count()/stored_volume().
	pub fn compact(&mut self) {
		if !self.occupied || self.is_leaf { return; }

		let root = self.levels[0].node_count().saturating_sub(1);
		let reachable = collect_reachable(&self.levels, root, DEPTH);
		let mut remaps: Vec<Vec<u32>> = (0..DEPTH)
			.map(|d| vec![0u32; self.levels[d].node_count() as usize])
			.collect();

		for d in (0..DEPTH).rev() {
			let child_remap = if d + 1 < DEPTH { Some(remaps[d + 1].as_slice()) } else { None };
			let (new_level, remap) = dedup_level(&self.levels[d], &reachable[d], child_remap, d + 1 == DEPTH);
			self.levels[d] = new_level;
			remaps[d] = remap;
		}
	}
}

fn collect_reachable(levels: &[Level], root: u32, depth: usize) -> Vec<Vec<u32>> {
	let mut reachable: Vec<Vec<u32>> = vec![Vec::new(); depth];
	if levels[0].node_count() == 0 { return reachable; }

	reachable[0].push(root);

	for d in 0..depth - 1 {
		let level = &levels[d];
		let mut next = Vec::new();
		for &n in &reachable[d] {
			let occ  = level.occupancy_mask[n as usize];
			let leaf = level.leaf_mask[n as usize];
			let base = level.children_offset[n as usize];
			let mut non_leaf = occ & !leaf;
			while non_leaf != 0 {
				let s    = non_leaf.trailing_zeros() as u8;
				let rank = (occ & ((1u64 << s) - 1)).count_ones();
				next.push(level.node_children.get(base + rank));
				non_leaf &= non_leaf - 1;
			}
		}
		next.sort_unstable();
		next.dedup();
		reachable[d + 1] = next;
	}

	reachable
}

fn node_sig(level: &Level, n: u32, child_remap: Option<&[u32]>, is_last: bool) -> Vec<u32> {
	let occ  = level.occupancy_mask[n as usize];
	let leaf = level.leaf_mask[n as usize];
	let base = level.children_offset[n as usize];

	let count = occ.count_ones() as usize;
	let mut sig = Vec::with_capacity(4 + count * 2);
	sig.push(occ as u32);
	sig.push((occ >> 32) as u32);
	sig.push(leaf as u32);
	sig.push((leaf >> 32) as u32);

	let mut mask = occ;
	let mut rank = 0u32;
	while mask != 0 {
		let s   = mask.trailing_zeros() as u8;
		let val = level.values.get(base + rank);
		let child = if is_last || (leaf >> s) & 1 != 0 {
			0
		} else {
			let c = level.node_children.get(base + rank);
			child_remap.map_or(c, |r| r[c as usize])
		};
		sig.push(child);
		sig.push(val);
		rank += 1;
		mask &= mask - 1;
	}
	sig
}

fn dedup_level(
	level: &Level,
	reachable: &[u32],
	child_remap: Option<&[u32]>,
	is_last: bool,
) -> (Level, Vec<u32>) {
	let mut remap = vec![0u32; level.node_count() as usize];
	let mut sig_to_new: HashMap<Vec<u32>, u32> = HashMap::new();
	let mut out = Level::new();

	for &old in reachable {
		let sig = node_sig(level, old, child_remap, is_last);
		let new_idx = if let Some(&existing) = sig_to_new.get(&sig) {
			existing
		} else {
			let idx = out.node_count();
			let occ  = level.occupancy_mask[old as usize];
			let leaf = level.leaf_mask[old as usize];
			let base = level.children_offset[old as usize];
			let offset = out.children_len();

			let mut mask = occ;
			let mut rank = 0u32;
			while mask != 0 {
				let s   = mask.trailing_zeros() as u8;
				let val = level.values.get(base + rank);
				let child = if is_last || (leaf >> s) & 1 != 0 {
					0
				} else {
					let c = level.node_children.get(base + rank);
					child_remap.map_or(c, |r| r[c as usize])
				};
				out.push_child(child, val);
				rank += 1;
				mask &= mask - 1;
			}
			out.push_node(occ, leaf, offset);
			sig_to_new.insert(sig, idx);
			idx
		};
		remap[old as usize] = new_idx;
	}

	(out, remap)
}
