use crate::import::VoxelSample;

/// Remove any voxel whose all 6 face-neighbors are occupied and non-transparent.
/// These are never visible from any direction.
pub fn cull_interior(samples: &mut Vec<VoxelSample>) {
	todo!()
}
