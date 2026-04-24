use super::Chunk;
use crate::{shape::Shape, tree::Aabb};

impl Chunk {
	// Build a new chunk from scratch using the AABB coverage walk over the given shapes.
	// lod controls voxel scale: LOD-0 leaves are 1^3 voxels, LOD-k leaves are 4^k voxels.
	pub fn build_from_shapes(lod: u8, chunk_aabb: Aabb, shapes: &[&dyn Shape]) -> Self {
		todo!()
	}

	// Partially rebuild only the region covered by region_aabb, leaving the rest untouched.
	pub fn rebuild_region(&mut self, lod: u8, region_aabb: Aabb, shapes: &[&dyn Shape]) {
		todo!()
	}
}
