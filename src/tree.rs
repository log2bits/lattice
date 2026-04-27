mod compact;
mod edit;
pub mod lod;
mod level;
mod stats;
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

}

