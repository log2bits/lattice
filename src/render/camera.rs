#![allow(unused)]

// Camera state: position, orientation, and projection parameters.
pub struct Camera {
	pub position: [f32; 3],
	pub forward: [f32; 3],
	pub up: [f32; 3],
	pub fov_y: f32, // vertical field of view in radians
}

impl Camera {
	pub fn new(position: [f32; 3], target: [f32; 3], fov_y: f32) -> Self {
		todo!()
	}

	// Returns the ray origin and direction for a pixel at (x, y) in [0,1]^2.
	pub fn ray(&self, x: f32, y: f32, aspect: f32) -> ([f32; 3], [f32; 3]) {
		todo!()
	}

	// Packed uniform data for the GPU shader.
	pub fn to_gpu_uniforms(&self, width: u32, height: u32) -> CameraUniforms {
		todo!()
	}
}

// Packed uniform buffer layout matching the WGSL camera struct.
#[repr(C)]
pub struct CameraUniforms {
	pub origin: [f32; 4],
	pub forward: [f32; 4],
	pub right: [f32; 4],
	pub up: [f32; 4],
}
