#![allow(unused)]

// Debug overlay passes: visualize normals, voxel depth, occupancy, etc.

pub enum DebugMode {
	None,
	Normals,
	Depth,
	VoxelIndex,
	Occupancy,
}

pub struct DebugOverlay {
	pub mode: DebugMode,
}

impl DebugOverlay {
	pub fn new(mode: DebugMode) -> Self {
		Self { mode }
	}

	// Returns GPU uniform data for the debug shader.
	pub fn to_uniforms(&self) -> u32 {
		todo!()
	}
}
