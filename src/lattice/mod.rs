use std::collections::HashMap;

pub mod bitpacked;
pub mod lut;
pub mod node;
pub mod voxel;

pub use bitpacked::BitpackedArray;
pub use lut::Lut;
pub use node::{LEAF_FLAG, child_count, is_leaf, leaf_value, make_leaf};
pub use voxel::{ColorPalette, Voxel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayerType {
	Grid,
	GeometryDag,
	MaterialDag,
}

// Configuration for one section of a Lattice. A section is a consecutive
// group of levels sharing the same layer type.
pub struct SectionConfig {
	pub layer: LayerType,
	pub num_levels: u8,
	pub lut: bool,
}

impl SectionConfig {
	pub fn grid(num_levels: u8) -> Self {
		Self {
			layer: LayerType::Grid,
			num_levels,
			lut: false,
		}
	}

	pub fn geometry_dag(num_levels: u8) -> Self {
		Self {
			layer: LayerType::GeometryDag,
			num_levels,
			lut: false,
		}
	}

	pub fn material_dag(num_levels: u8) -> Self {
		Self {
			layer: LayerType::MaterialDag,
			num_levels,
			lut: false,
		}
	}

	pub fn with_lut(mut self) -> Self {
		self.lut = true;
		self
	}
}

// One interior level of the 64-tree. SoA layout: each field is a separate
// contiguous array indexed by node index.
pub struct Level {
	pub occupancy: Vec<u64>,
	pub voxel_count: Option<Vec<u32>>, // Geometry DAG levels only
	pub children_start: Vec<u32>,
	pub children: BitpackedArray,
	pub(crate) dedup: HashMap<u64, u32>, // build-time only, keyed on node hash
}

impl Level {
	pub fn new(layer: LayerType) -> Self {
		Self {
			occupancy: Vec::new(),
			voxel_count: match layer {
				LayerType::GeometryDag => Some(Vec::new()),
				_ => None,
			},
			children_start: Vec::new(),
			children: BitpackedArray::new(),
			dedup: HashMap::new(),
		}
	}

	pub fn len(&self) -> u32 {
		self.occupancy.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.occupancy.is_empty()
	}

	// Returns the children slice for a given node index.
	pub fn children_of(&self, node_idx: u32) -> Box<dyn Iterator<Item = u32> + '_> {
		todo!()
	}

	// Inserts a node and returns its index, deduplicating by (occupancy, children).
	// For GeometryDag levels, dedup ignores children (geometry-only hash).
	pub fn insert(
		&mut self,
		layer: LayerType,
		occupancy: u64,
		voxel_count: u32,
		children: &[u32],
	) -> u32 {
		todo!()
	}
}

// Per-section-root LUT. Owned by the section root node and shared across all
// instances via DAG node sharing.
pub struct SectionRoot {
	pub root_node_index: u32,
	pub lut_index_bits: u8,    // bit width for in-tree leaf entries
	pub lut_entries: Vec<u32>, // global voxel LUT indices
	pub leaf_offset: u64,      // byte offset into the bottom level's children
}

// One section: a group of same-type levels plus optional per-section-root LUTs
// and (for Geometry DAG sections) a Dolonius materials array.
pub struct Section {
	pub config: SectionConfig,
	pub levels: Vec<Level>,
	pub roots: Vec<SectionRoot>,           // populated if config.lut
	pub materials: Option<BitpackedArray>, // Geometry DAG sections only
}

// The fully-built lattice. All voxel references in the tree are indices into
// voxel_lut. voxel_bits is the bit width used for those indices throughout.
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
			// top: spatial index into block DAGs
			SectionConfig::grid(1),
			// middle: block geometry dedup across the world
			SectionConfig::geometry_dag(3).with_lut(),
			// bottom: voxel color dedup within each block
			SectionConfig::material_dag(2).with_lut(),
		]);
	}
}
