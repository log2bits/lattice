#![allow(unused)]
use crate::lattice::Lattice;

// GPU-resident lattice data. All CPU-side Vecs have been uploaded to wgpu
// buffers. The Lattice itself is dropped after upload.
pub struct GpuLattice {
	pub occupancy_bufs: Vec<wgpu::Buffer>,      // one per level
	pub voxel_count_bufs: Vec<wgpu::Buffer>,    // Geometry DAG levels only
	pub children_start_bufs: Vec<wgpu::Buffer>, // one per level
	pub children_bufs: Vec<wgpu::Buffer>,       // one per level, bitpacked
	pub section_root_bufs: Vec<wgpu::Buffer>,   // one per section with LUT
	pub materials_bufs: Vec<wgpu::Buffer>,      // one per Geometry DAG section
	pub voxel_lut_buf: wgpu::Buffer,
	pub palette_buf: wgpu::Buffer,
	pub voxel_bits: u8,
}

// Uploads a fully-built Lattice to GPU buffers.
pub fn upload(lattice: Lattice, device: &wgpu::Device, queue: &wgpu::Queue) -> GpuLattice {
	todo!()
}
