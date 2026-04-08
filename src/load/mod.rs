#![allow(unused)]
pub mod header;
pub mod stream;
pub mod upload;

use crate::lattice::Lattice;
use std::path::Path;
use upload::GpuLattice;

// Loads a .lattice file into RAM as a full Lattice. All chunks are loaded at
// full depth.
pub fn load_lattice(path: &Path) -> Result<Lattice, anyhow::Error> {
	todo!()
}

// Uploads a chunk to the GPU at the specified depth, for streaming and LOD.
// depth == lattice.depth means full detail. depth < lattice.depth means the
// bottom (lattice.depth - depth) levels are replaced with LEAF_FLAG entries
// carrying rep_material LUT indices.
pub fn upload_chunk(
	lattice: &Lattice,
	chunk_index: u32,
	depth: u8,
	device: &wgpu::Device,
	queue: &wgpu::Queue,
	gpu: &mut GpuLattice,
) -> Result<(), anyhow::Error> {
	todo!()
}
