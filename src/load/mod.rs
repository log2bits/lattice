#![allow(unused)]
pub mod header;
pub mod stream;
pub mod upload;

use crate::lattice::Lattice;
use std::path::Path;
use upload::GpuLattice;

// Loads a .lattice file into RAM as a full Lattice. All roots are loaded at
// full depth. rep_voxels are computed from the materials data immediately after
// loading and before any VRAM upload.
pub fn load_lattice(path: &Path) -> Result<Lattice, anyhow::Error> {
	todo!()
}

// Uploads a subset of roots to the GPU at the specified depth, for streaming
// and LOD. depth == lattice.dag_depth means full detail. depth < dag_depth
// means the bottom (dag_depth - depth) levels are replaced with LEAF_FLAG
// entries carrying rep_voxel LUT indices.
pub fn upload_root(
	lattice: &Lattice,
	root_index: u32,
	depth: u8,
	device: &wgpu::Device,
	queue: &wgpu::Queue,
	gpu: &mut GpuLattice,
) -> Result<(), anyhow::Error> {
	todo!()
}
