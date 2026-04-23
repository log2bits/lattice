mod build;
mod compact;
mod edit;
mod traverse;

pub use traverse::{Ray, RayHit};

use crate::bitpacked::BitpackedArray;

#[derive(Clone, Copy, Debug)]
pub struct Aabb {
	pub min: [i64; 3],
	pub max: [i64; 3],
}

impl Aabb {
	pub fn contains(&self, other: &Aabb) -> bool { todo!() }
	pub fn overlaps(&self, other: &Aabb) -> bool { todo!() }
	pub fn split_at_slot(&self, slot: u32) -> Aabb { todo!() }
}

pub struct Level {
	pub occupancy_mask: Vec<u64>,
	pub terminal_mask: Vec<u64>,
	pub children_offset: Vec<u32>,
	pub node_children: BitpackedArray,
	pub values: BitpackedArray,
}

pub struct Tree {
	pub root: u32,
	pub levels: Vec<Level>,
}

impl Level {
	pub fn node_count(&self) -> u32 {
		self.occupancy_mask.len() as u32
	}

	pub fn is_occupied(&self, node: u32, slot: u32) -> bool {
		(self.occupancy_mask[node as usize] >> slot) & 1 != 0
	}

	pub fn is_terminal(&self, node: u32, slot: u32) -> bool {
		(self.terminal_mask[node as usize] >> slot) & 1 != 0
	}

	// Packed index into node_children and values for the child at slot.
	pub fn child_idx(&self, node: u32, slot: u32) -> u32 {
		let rank = (self.occupancy_mask[node as usize] & ((1u64 << slot) - 1)).count_ones();
		self.children_offset[node as usize] + rank
	}
}

impl Tree {
	pub fn with_depth(depth: u8) -> Self { todo!() }

	pub fn depth(&self) -> u8 {
		self.levels.len() as u8
	}

	// Side length in leaf voxels: 4^depth.
	pub fn side_len(&self) -> u32 {
		4u32.pow(self.depth() as u32)
	}
}
