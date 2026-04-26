mod compact;
mod edit;
mod level;
mod traverse;

use std::array::from_fn;

pub use edit::{Edit, EditPacket, OrderedEdits, TreePath, DELETE};
pub use level::Level;
pub use traverse::{Ray, RayHit};

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
	pub min: [i64; 3],
	pub max: [i64; 3],
}

impl Aabb {
	pub fn contains(&self, other: &Aabb) -> bool {
		(0..3).all(|i| self.min[i] <= other.min[i] && self.max[i] >= other.max[i])
	}
	pub fn overlaps(&self, other: &Aabb) -> bool {
		(0..3).all(|i| self.min[i] < other.max[i] && self.max[i] > other.min[i])
	}
	pub fn split_at_slot(&self, slot: u32) -> Aabb {
		let [x, y, z] = [slot & 3, (slot >> 2) & 3, slot >> 4].map(|v| v as i64);
		let [sx, sy, sz] = [0, 1, 2].map(|i| (self.max[i] - self.min[i]) >> 2);
		let min = [
			self.min[0] + x * sx,
			self.min[1] + y * sy,
			self.min[2] + z * sz,
		];
		let max = [min[0] + sx, min[1] + sy, min[2] + sz];
		Aabb { min, max }
	}
}

#[derive(Clone)]
pub struct Tree<const DEPTH: usize> {
	pub occupied: bool,
	pub is_leaf: bool,
	pub value: u32,
	pub leaf_size: u64,
	pub edits: OrderedEdits<DEPTH>,
	// occupied/is_leaf/value represent the root above all levels.
	// levels[0] = root node (1 node after compact; its slot-children point into levels[1])
	// levels[d] = nodes at tree depth d; levels[DEPTH-1] = deepest nodes, whose slots are individual cells
	pub levels: [Level; DEPTH],
}

impl<const DEPTH: usize> Tree<DEPTH> {
	pub fn new(leaf_size: u64) -> Self {
		Self {
			occupied: false,
			is_leaf: false,
			value: 0,
			leaf_size,
			edits: Default::default(),
			levels: from_fn(|_| Level::new()),
		}
	}

	pub fn depth(&self) -> usize {
		DEPTH
	}

	// Side length in cells: 4^DEPTH.
	pub fn side_len(&self) -> u32 {
		4u32.pow(DEPTH as u32)
	}

	pub fn bytes(&self) -> usize {
		self.levels.iter().map(|l| l.bytes()).sum()
	}

	// Leaves physically stored (unique nodes only, no SVDAG path-following).
	pub fn unique_leaf_count(&self) -> u64 {
		if self.is_leaf { return 1; }
		self.levels.iter().map(|l| l.leaf_count()).sum()
	}

	// Volume physically stored (unique nodes only).
	pub fn unique_volume(&self) -> u64 {
		if self.is_leaf { return (self.leaf_size * self.side_len() as u64).pow(3); }
		let mut total = 0u64;
		for d in 0..DEPTH {
			let side = self.leaf_size * 4u64.pow((DEPTH - d - 1) as u32);
			total += self.levels[d].leaf_count() * side * side * side;
		}
		total
	}

	// Counts represented leaves, following SVDAG sharing (same node reached via multiple paths counts multiple times).
	pub fn leaf_count(&self) -> u64 {
		if !self.occupied { return 0; }
		if self.is_leaf { return 1; }
		if self.levels[0].node_count() == 0 { return 0; }
		let root = self.levels[0].node_count() - 1;
		geo_leaf_count::<DEPTH>(&self.levels, 0, root)
	}

	// Total cells covered by leaves, following SVDAG sharing.
	// A leaf slot at depth d covers (leaf_size * 4^(DEPTH-d-1))^3 cells.
	pub fn stored_volume(&self) -> u64 {
		if !self.occupied { return 0; }
		if self.is_leaf {
			return (self.leaf_size * self.side_len() as u64).pow(3);
		}
		if self.levels[0].node_count() == 0 { return 0; }
		let root = self.levels[0].node_count() - 1;
		geo_stored_volume::<DEPTH>(&self.levels, 0, root, self.leaf_size)
	}
}

fn geo_leaf_count<const DEPTH: usize>(levels: &[Level], d: usize, node: u32) -> u64 {
	let level = &levels[d];
	let occ  = level.occupancy_mask[node as usize];
	let leaf = level.leaf_mask[node as usize];
	let base = level.children_offset[node as usize];
	let is_leaf_level = d + 1 == DEPTH;
	let mut count = 0u64;
	let mut mask = occ;
	while mask != 0 {
		let s    = mask.trailing_zeros() as usize;
		let rank = (occ & ((1u64 << s) - 1)).count_ones();
		if (leaf >> s) & 1 != 0 || is_leaf_level {
			count += 1;
		} else {
			let child = level.node_children.get(base + rank);
			count += geo_leaf_count::<DEPTH>(levels, d + 1, child);
		}
		mask &= mask - 1;
	}
	count
}

fn geo_stored_volume<const DEPTH: usize>(levels: &[Level], d: usize, node: u32, leaf_size: u64) -> u64 {
	let level = &levels[d];
	let occ  = level.occupancy_mask[node as usize];
	let leaf = level.leaf_mask[node as usize];
	let base = level.children_offset[node as usize];
	let is_leaf_level = d + 1 == DEPTH;
	let side = leaf_size * 4u64.pow((DEPTH - d - 1) as u32);
	let mut total = 0u64;
	let mut mask = occ;
	while mask != 0 {
		let s    = mask.trailing_zeros() as usize;
		let rank = (occ & ((1u64 << s) - 1)).count_ones();
		if (leaf >> s) & 1 != 0 || is_leaf_level {
			total += side * side * side;
		} else {
			let child = level.node_children.get(base + rank);
			total += geo_stored_volume::<DEPTH>(levels, d + 1, child, leaf_size);
		}
		mask &= mask - 1;
	}
	total
}
