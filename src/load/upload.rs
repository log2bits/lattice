#![allow(unused)]
use crate::lattice::Lattice;

// GPU-resident lattice data. All CPU-side Vecs have been uploaded to wgpu
// buffers. The Lattice itself is dropped after upload.
pub struct GpuLattice {
	pub child_mask_bufs: Vec<wgpu::Buffer>,    // one per level
	pub child_start_bufs: Vec<wgpu::Buffer>,   // one per level
	pub rep_material_bufs: Vec<wgpu::Buffer>,  // one per level
	pub children_bufs: Vec<wgpu::Buffer>,      // one per level, bitpacked
	pub lut_bufs: Vec<wgpu::Buffer>,           // one per chunk
	pub grid_buf: wgpu::Buffer,
}

// Uploads a fully-built Lattice to GPU buffers.
pub fn upload(lattice: Lattice, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuLattice {
	todo!()
}
