#![allow(unused)]

// Frame loop and pass orchestration. No rendering logic lives here.
// Each frame: dispatch primary rays, dispatch GI bounces, dispatch
// accumulation, present output. All computation is in the WGSL shaders.

use super::camera::{Camera, CameraUniforms};
use super::debug::DebugOverlay;
use super::gi::GiPass;
use super::traverse::TraversalPass;
use crate::load::upload::GpuLattice;

pub struct Renderer {
	device: wgpu::Device,
	queue: wgpu::Queue,
	camera: Camera,
	traversal: TraversalPass,
	gi: GiPass,
	debug: DebugOverlay,
	output: wgpu::Texture,
	width: u32,
	height: u32,
}

impl Renderer {
	pub fn new(device: wgpu::Device, queue: wgpu::Queue, lattice: GpuLattice, width: u32, height: u32) -> Self {
		todo!()
	}

	// Dispatches one frame: traversal -> GI -> accumulation -> (debug overlay).
	pub fn render(&mut self) {
		todo!()
	}

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
