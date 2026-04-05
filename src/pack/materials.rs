#![allow(unused)]
use crate::lattice::{BitpackedArray, Voxel};
use std::path::Path;

// Streams the Dolonius materials array for a Geometry DAG section to a temp
// file during construction, then finalizes it as a BitpackedArray of global
// voxel LUT indices at voxel_bits.
pub struct MaterialsWriter {
	path: std::path::PathBuf,
}

impl MaterialsWriter {
	pub fn new(tmp: &Path) -> Self {
		todo!()
	}

	// Appends one voxel to the materials stream.
	pub fn push(&mut self, voxel: Voxel) {
		todo!()
	}

	// Finishes writing, maps all voxels to global voxel LUT indices,
	// and returns the bitpacked result.
	pub fn finalize(self, voxel_lut: &[Voxel], voxel_bits: u8) -> BitpackedArray {
		todo!()
	}
}
