pub struct RenderPipeline {
	pipeline: wgpu::RenderPipeline,
	bind_group_layout: wgpu::BindGroupLayout,
}

impl RenderPipeline {
	pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
		todo!()
	}
	pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
		&self.bind_group_layout
	}
}
