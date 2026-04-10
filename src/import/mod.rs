pub mod cull;
pub mod gltf;
pub mod palette;
pub mod partition;
pub mod pbr;
pub mod voxelize;

use crate::voxel::Voxel;

pub struct ImportConfig {
	pub voxel_size: f32,
	pub depth: u8,
	pub palette_path: std::path::PathBuf,
}

/// One voxelized sample: morton-ordered position within a chunk plus voxel value.
pub struct VoxelSample {
	pub morton: u64,
	pub voxel: Voxel,
}

/// Run the full import pipeline: glTF -> per-chunk VoxelSample streams.
/// Calls on_chunk(chunk_grid_index, samples) for each non-empty chunk.
pub fn import(path: &std::path::Path, config: &ImportConfig, on_chunk: impl FnMut(u64, Vec<VoxelSample>)) -> anyhow::Result<()> {
	todo!()
}
