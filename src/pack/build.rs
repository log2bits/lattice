use crate::import::VoxelSample;
use crate::tree::chunk::Chunk;

/// Build a chunk tree bottom-up from morton-sorted samples.
/// Creates leaf nodes first, then parent nodes from groups of 64.
/// Uniform subtrees collapse into a single leaf_materials entry in the parent.
/// lod_material is computed by blending children bottom-up.
pub fn build_chunk(samples: &[VoxelSample], depth: u8) -> Chunk {
	todo!()
}
