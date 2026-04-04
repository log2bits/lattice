#![allow(unused)]

// Traversal pass orchestration. The actual 64-tree DDA, LUT decoding, and
// Dolonius offset accumulation all live in traverse.wgsl. This file manages
// the pipeline, bind groups, and the hit buffer the traversal writes into.

use crate::load::upload::GpuLattice;

// Buffer written by the traversal pass. One entry per pixel: the hit voxel
// index, face, and ray parameter t. Read by the GI pass on the same frame.
pub struct HitBuffer {
  pub buf:          wgpu::Buffer,
  pub pixel_count:  u64,
}

impl HitBuffer {
  pub fn new(device: &wgpu::Device, width: u32, height: u32) -> Self {
    todo!()
  }

  pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
    todo!()
  }
}

// Pipeline and bind group for the traversal compute pass.
pub struct TraversalPass {
  pub pipeline:   wgpu::ComputePipeline,
  pub bind_group: wgpu::BindGroup,
  pub hit_buf:    HitBuffer,
}

impl TraversalPass {
  pub fn new(device: &wgpu::Device, lattice: &GpuLattice, width: u32, height: u32) -> Self {
    todo!()
  }

  pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, width: u32, height: u32) {
    todo!()
  }
}
