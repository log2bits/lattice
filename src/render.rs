pub mod camera;
mod pipeline;
mod present;
mod upload;

pub use camera::CameraPos;

use crate::world::World;

pub struct Renderer {
	device: wgpu::Device,
	queue: wgpu::Queue,
	surface: wgpu::Surface<'static>,
	config: wgpu::SurfaceConfiguration,
	pipeline: pipeline::RenderPipeline,
	world_tree_buf: wgpu::Buffer,
	chunk_offsets_buf: wgpu::Buffer,
	chunk_data_buf: wgpu::Buffer,
}

impl Renderer {
	pub async fn new(window: std::sync::Arc<winit::window::Window>) -> Self {
		todo!()
	}
	pub fn render(&mut self, world: &World, camera: &CameraPos) {
		todo!()
	}
	pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
		todo!()
	}
}
