pub mod palette;
pub mod gltf;

use crate::lattice::Voxel;

// One voxelized surface sample. Position is in integer voxel-space coordinates.
// The importer produces these in Morton order, ready for the pack stage.
pub struct VoxelSample {
  pub position: [i64; 3],
  pub voxel:    Voxel,
}

// Parameters shared across all importers.
pub struct ImportConfig {
  pub voxel_size: f64,
  pub world_min:  [i64; 3],
  pub world_max:  [i64; 3],
}
