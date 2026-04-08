pub mod bitpacked;
pub mod grid;
pub mod lut;
pub mod node;
pub mod svo;
pub mod voxel;

pub use bitpacked::BitpackedArray;
pub use grid::Grid;
pub use lut::Lut;
pub use node::{LEAF_FLAG, child_count, is_leaf, leaf_value, make_leaf};
pub use svo::{Chunk, Level};
pub use voxel::Voxel;

// Stack-allocated iterator over the children of a node.
pub struct ChildIter<'a> {
	arr: &'a BitpackedArray,
	pos: u32,
	end: u32,
}

impl<'a> ChildIter<'a> {
	pub(crate) fn new(arr: &'a BitpackedArray, start: u32, end: u32) -> Self {
		Self { arr, pos: start, end }
	}
}

impl<'a> Iterator for ChildIter<'a> {
	type Item = u32;

	fn next(&mut self) -> Option<u32> {
		if self.pos == self.end {
			return None;
		}
		let v = self.arr.get(self.pos);
		self.pos += 1;
		Some(v)
	}
}

// The fully-built in-memory lattice. One grid of chunk pointers, all sharing
// the same pool of SVO levels. Chunks are loaded from disk as full trees and
// kept in RAM. VRAM gets partial-depth uploads based on camera distance.
//
// depth: number of SVO levels. The tree covers (4^depth)^3 voxels per chunk.
// Depth 3 = 64^3 voxels, depth 4 = 256^3, depth 5 = 1024^3.
pub struct Lattice {
	pub grid: Grid,
	pub depth: u8,
	pub levels: Vec<Level>,  // shared node pools, one per depth level
	pub chunks: Vec<Chunk>,
}

impl Lattice {
	pub fn new(depth: u8, grid_dims: [u32; 3]) -> Self {
		Self {
			grid: Grid::new(grid_dims),
			depth,
			levels: (0..depth).map(|_| Level::new()).collect(),
			chunks: Vec::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn build_empty_lattice() {
		let lattice = Lattice::new(3, [16, 16, 16]);
		assert_eq!(lattice.depth, 3);
		assert_eq!(lattice.levels.len(), 3);
	}
}
