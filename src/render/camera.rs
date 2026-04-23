use crate::tree::Ray;

pub struct CameraPos {
	pub chunk: [i64; 3],  // LOD-0 chunk coordinates
	pub local: [f32; 3],  // offset within chunk, always in [0, chunk_side_len)
	pub yaw: f32,
	pub pitch: f32,
}

impl CameraPos {
	pub fn ray(&self) -> Ray { todo!() }
}
