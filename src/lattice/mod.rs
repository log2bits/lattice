pub mod bitpacked;
pub mod geometry_dag;
pub mod grid;
pub mod lut;
pub mod node;
pub mod voxel;

pub use bitpacked::BitpackedArray;
pub use geometry_dag::{GeometryDagLevel, GeometryDagRoot};
pub use grid::GridLevel;
pub use lut::{Lut, MaterialsArray};
pub use node::{LEAF_FLAG, child_count, is_leaf, leaf_value, make_leaf};
pub use voxel::{ColorPalette, Voxel};

// Stack-allocated iterator over the children of a DAG node.
pub struct ChildIter<'a> {
	arr: &'a BitpackedArray,
	pos: u32,
	end: u32,
}

impl<'a> ChildIter<'a> {
	pub(crate) fn new(arr: &'a BitpackedArray, start: u32, end: u32) -> Self {
		Self {
			arr,
			pos: start,
			end,
		}
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

// The fully-built in-memory lattice. One grid of DAG roots, all sharing the
// same pool of geometry levels. Roots are loaded from disk as full trees and
// kept in RAM. VRAM gets partial-depth uploads based on camera distance.
//
// dag_depth: number of geometry levels. The tree covers (4^dag_depth)^3 voxels
// per root. Depth 3 = 64^3 voxels, depth 4 = 256^3, depth 5 = 1024^3.
pub struct Lattice {
	pub grid: GridLevel,
	pub dag_depth: u8,
	pub levels: Vec<GeometryDagLevel>, // shared geometry pool, dag_depth levels
	pub roots: Vec<GeometryDagRoot>,
	pub palette: ColorPalette,
}

impl Lattice {
	pub fn new(dag_depth: u8) -> Self {
		Self {
			grid: GridLevel::new(),
			dag_depth,
			levels: (0..dag_depth).map(|_| GeometryDagLevel::new()).collect(),
			roots: Vec::new(),
			palette: ColorPalette::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn build_empty_lattice() {
		let lattice = Lattice::new(3);
		assert_eq!(lattice.dag_depth, 3);
		assert_eq!(lattice.levels.len(), 3);
	}
}
