pub mod build;
pub mod finalize;
pub mod sort;
use crate::import::VoxelSample;

pub struct PackConfig {
	pub depth: u8,
	pub voxel_size: f32,
}

/// Create a Packer that streams chunk data into a .lattice file at path.
pub fn pack(config: PackConfig, path: &std::path::Path) -> anyhow::Result<Packer> {
	todo!()
}

pub struct Packer {
	config: PackConfig,
	out: std::fs::File,
}

impl Packer {
	/// Add one chunk's morton-sorted VoxelSample stream.
	pub fn add_chunk(&mut self, chunk_index: u64, samples: Vec<VoxelSample>) -> anyhow::Result<()> {
		todo!()
	}

	/// Finalize and flush the .lattice file.
	pub fn finish(self) -> anyhow::Result<()> {
		todo!()
	}
}
