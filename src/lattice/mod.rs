pub mod bitpacked;
pub mod geometry_dag;
pub mod grid;
pub mod lut;
pub mod material_dag;
pub mod node;
pub mod voxel;

pub use bitpacked::BitpackedArray;
pub use geometry_dag::{GeometryDagLevel, GeometryDagRoot};
pub use grid::GridLevel;
pub use lut::Lut;
pub use material_dag::{MaterialDagLevel, MaterialDagRoot};
pub use node::{LEAF_FLAG, child_count, is_leaf, leaf_value, make_leaf};
pub use voxel::{ColorPalette, Voxel};

// Stack-allocated iterator over the children of a DAG node. Returned by
// children_of() on GeometryDagLevel and MaterialDagLevel.
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

// Configuration for one section. Passed to Lattice::new to describe the
// desired structure before any data is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerType {
	Grid,
	GeometryDag,
	MaterialDag,
}

pub struct SectionConfig {
	pub layer: LayerType,
	pub num_levels: u8,
	pub lut: bool,
}

impl SectionConfig {
	pub fn grid(num_levels: u8) -> Self {
		Self { layer: LayerType::Grid, num_levels, lut: false }
	}

	pub fn geometry_dag(num_levels: u8) -> Self {
		Self { layer: LayerType::GeometryDag, num_levels, lut: false }
	}

	pub fn material_dag(num_levels: u8) -> Self {
		Self { layer: LayerType::MaterialDag, num_levels, lut: false }
	}

	pub fn with_lut(mut self) -> Self {
		self.lut = true;
		self
	}
}

// The data for one section. Each variant carries exactly the types and fields
// it needs -- no Options, no unused fields, no shared root struct that forces
// a common layout on different section types.
//
// To add a new layer type (e.g. SSVDAG), add a new file with the level and root
// structs, then add a variant here. The compiler will point out every match arm
// that needs updating.
pub enum SectionData {
	Grid(GridLevel),
	GeometryDag {
		levels: Vec<GeometryDagLevel>,
		roots: Vec<GeometryDagRoot>,
	},
	MaterialDag {
		levels: Vec<MaterialDagLevel>,
		roots: Vec<MaterialDagRoot>,
	},
}

pub struct Section {
	pub config: SectionConfig,
	pub data: SectionData,
}

// The fully-built lattice. All voxel references in the tree resolve through
// voxel_lut to a fixed 32-bit format. voxel_bits is the bit width used for
// those indices throughout the tree.
pub struct Lattice {
	pub sections: Vec<Section>,
	pub voxel_lut: Vec<Voxel>,
	pub voxel_bits: u8,
	pub palette: ColorPalette,
}

impl Lattice {
	pub fn new(configs: Vec<SectionConfig>) -> Self {
		todo!()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn minecraft_lattice() {
		let _lattice = Lattice::new(vec![
			SectionConfig::grid(1),
			SectionConfig::geometry_dag(3).with_lut(),
			SectionConfig::material_dag(2).with_lut(),
		]);
	}
}
