pub mod camera;
pub mod debug;
pub mod lod;
pub mod pipeline;
pub mod present;
pub mod upload;

pub use camera::Camera;

use crate::tree::Lattice;

pub struct Renderer {
	// device, queue, surface, buffers -- wgpu state lives here
}

impl Renderer {
	pub fn new(window: &winit::window::Window, lattice: &Lattice) -> anyhow::Result<Self> {
		todo!()
	}

	pub fn render(&mut self, camera: &Camera) -> anyhow::Result<()> {
		todo!()
	}

	pub fn resize(&mut self, width: u32, height: u32) {
		todo!()
	}
}
