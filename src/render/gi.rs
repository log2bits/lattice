#![allow(unused)]

// GI pass orchestration. All path tracing and per-face accumulation logic
// lives in gi.wgsl and accumulate.wgsl. This file manages the pipelines,
// bind groups, and the per-face lighting buffer on the GPU.

use crate::load::upload::GpuLattice;

// Per-face lighting buffer. Stores one accumulated radiance value per voxel
// face. Indexed by (node_idx * 6 + face_idx). Lives entirely on the GPU;
// the CPU never reads it back except for debug captures.
pub struct LightingBuffer {
  pub buf:        wgpu::Buffer,
  pub face_count: u64,
}

impl LightingBuffer {
  pub fn new(device: &wgpu::Device, face_count: u64) -> Self {
    todo!()
  }
}

// Pipelines and bind groups for the GI and accumulation passes.
pub struct GiPass {
  pub gi_pipeline:          wgpu::ComputePipeline,
  pub accumulate_pipeline:  wgpu::ComputePipeline,
  pub bind_group:           wgpu::BindGroup,
  pub lighting_buf:         LightingBuffer,
}

impl GiPass {
  pub fn new(device: &wgpu::Device, lattice: &GpuLattice, face_count: u64) -> Self {
    todo!()
  }

  pub fn dispatch(&self, encoder: &mut wgpu::CommandEncoder, width: u32, height: u32) {
    todo!()
  }
}
