#![allow(unused)]
use crate::load::upload::GpuLattice;
use super::camera::Camera;

// Top-level renderer. Owns the wgpu pipelines and per-frame state.
pub struct Renderer {
  device:   wgpu::Device,
  queue:    wgpu::Queue,
  lattice:  GpuLattice,
  camera:   Camera,
  output:   wgpu::Texture,
}

impl Renderer {
  pub fn new(device: wgpu::Device, queue: wgpu::Queue, lattice: GpuLattice, width: u32, height: u32) -> Self {
    todo!()
  }

  // Dispatches one frame: primary rays, GI bounce loop, accumulation.
  pub fn render(&mut self) {
    todo!()
  }

  // Returns the current output texture view for presentation.
  pub fn output_view(&self) -> wgpu::TextureView {
    todo!()
  }

  pub fn resize(&mut self, width: u32, height: u32) {
    todo!()
  }

  pub fn set_camera(&mut self, camera: Camera) {
    self.camera = camera;
  }
}
