/// Orbit camera with mouse/keyboard input.
pub struct Camera {
	pub position: [f32; 3],
	pub target: [f32; 3],
	pub fov_y: f32,
	pub near: f32,
	pub far: f32,
}

// Packed uniform buffer layout matching the WGSL camera struct.
#[repr(C)]
pub struct CameraUniforms {
	pub origin: [f32; 4],
	pub forward: [f32; 4],
	pub right: [f32; 4],
	pub up: [f32; 4],
}

impl Default for Camera {
	fn default() -> Self {
		Self::new()
	}
}

impl Camera {
	pub fn new() -> Self {
		todo!()
	}

	pub fn view_matrix(&self) -> [[f32; 4]; 4] {
		todo!()
	}

	pub fn proj_matrix(&self, aspect: f32) -> [[f32; 4]; 4] {
		todo!()
	}

	pub fn to_gpu_uniforms(&self, width: u32, height: u32) -> CameraUniforms {
		todo!()
	}

	pub fn handle_input(&mut self, event: &winit::event::WindowEvent) {
		todo!()
	}
}
