use std::collections::HashMap;

use super::{BitpackedArray, ChildIter, MaterialsArray, Voxel};

// One level of the geometry DAG. Deduplicates on occupancy only -- two nodes
// with the same shape but different materials underneath share a node. Material
// data is tracked separately in each root's MaterialsArray via the Dolonius
// running offset.
pub struct GeometryDagLevel {
	pub occupancy: Vec<u64>,
	pub voxel_count: Vec<u32>,
	pub children_start: Vec<u32>,
	pub children: BitpackedArray,
	pub(crate) lookup: HashMap<BitpackedArray, u32>, // build-time only
}

impl GeometryDagLevel {
	pub fn new() -> Self {
		Self {
			occupancy: Vec::new(),
			voxel_count: Vec::new(),
			children_start: Vec::new(),
			children: BitpackedArray::new(),
			lookup: HashMap::new(),
		}
	}

	pub fn len(&self) -> u32 {
		self.occupancy.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.occupancy.is_empty()
	}

	pub fn children_of(&self, node_idx: u32) -> ChildIter<'_> {
		let start = self.children_start[node_idx as usize];
		let count = self.occupancy[node_idx as usize].count_ones();
		ChildIter::new(&self.children, start, start + count)
	}

	// Deduplicates on occupancy only. Children are stored but not part of the
	// hash key -- identical geometry is shared regardless of materials below.
	pub fn insert(&mut self, occupancy: u64, voxel_count: u32, children: &[u32]) -> u32 {
		todo!()
	}
}

impl Default for GeometryDagLevel {
	fn default() -> Self {
		Self::new()
	}
}

// One root of the geometry DAG. Owns all per-instance data for a subtree:
// the materials, and the precomputed rep_voxel for every internal node in
// DFS order (computed at load time from the materials, never stored on disk).
//
// leaf_start is the logical entry index into the bottom level's children
// BitpackedArray where this root's leaf entries begin. The on-disk format
// stores a byte offset; the conversion happens at serialization time.
//
// In VRAM, a root may be loaded at partial depth for LOD. At the truncation
// depth, child entries hold LEAF_FLAG | rep_voxel_lut_index instead of real
// pointers. The GPU traversal terminates there and returns the rep voxel.
pub struct GeometryDagRoot {
	pub root_node_index: u32,
	pub leaf_start: u32,
	pub materials: MaterialsArray,

	// One rep Voxel per node in DFS order, computed at load time. Covers both
	// internal nodes (blended from children) and leaves (direct material value).
	// Used by the GPU when traversal terminates early due to LOD.
	pub rep_voxels: Vec<Voxel>,
}

impl GeometryDagRoot {
	// Computes rep_voxels with a bottom-up pass over the geometry tree after
	// the root's data is loaded from disk. Must be called before VRAM upload.
	pub fn compute_rep_voxels(&mut self, levels: &[GeometryDagLevel]) {
		todo!()
	}
}
