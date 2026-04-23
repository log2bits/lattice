use super::{Aabb, Tree};

pub struct Ray {
	pub origin: [f32; 3],
	pub dir: [f32; 3],
}

pub struct RayHit {
	pub t: f32,
	pub normal: [i32; 3],
	pub value: u32,
}

impl Tree {
	// DDA traversal with ancestor stack. Returns the first occupied terminal
	// node hit by the ray and its packed value (material index or chunk handle).
	pub fn trace(&self, ray: &Ray, bounds: Aabb) -> Option<RayHit> { todo!() }
}
