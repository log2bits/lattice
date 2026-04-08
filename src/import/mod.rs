pub mod color;
pub mod gltf;

use std::collections::HashMap;

use crate::lattice::Voxel;

// One voxelized surface sample. Position is in integer voxel-space coordinates.
// The importer produces these in Morton order, ready for the pack stage.
pub struct VoxelSample {
	pub position: [i64; 3],
	pub voxel: Voxel,
}

// Parameters shared across all importers.
pub struct ImportConfig {
	pub voxel_size: f64,
	pub world_min: [i64; 3],
	pub world_max: [i64; 3],
	// Voxelization chunk size in voxels per side. Must be a power of 4.
	// Controls peak memory during import; has no effect on output structure.
	pub chunk_size: u32,
	// Precomputed OKLab palette image. Each unique pixel is one palette entry.
	pub palette_path: std::path::PathBuf,
}
