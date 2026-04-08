pub mod repack;
pub mod serialize;
pub mod sort;
pub mod tree;

use crate::import::VoxelSample;
use std::path::Path;

pub struct PackConfig {
	pub depth: u8,
	pub world_min: [i64; 3],
	pub world_max: [i64; 3],
}

pub struct Packer {
	config: PackConfig,
	out: std::path::PathBuf,
}

impl Packer {
	// Feed one Morton-sorted chunk of samples. Chunks must arrive in Morton order.
	pub fn add_chunk(&mut self, samples: Vec<VoxelSample>) {
		todo!()
	}

	// Finalize the SVO and write the .lattice file.
	pub fn finish(self) -> Result<(), anyhow::Error> {
		todo!()
	}
}

// Creates a Packer that builds a Lattice from a sorted VoxelSample stream and writes it to a .lattice file.
pub fn pack(config: PackConfig, out: &Path) -> Result<Packer, anyhow::Error> {
	Ok(Packer { config, out: out.to_path_buf() })
}
