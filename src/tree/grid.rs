/// Flat 3D array of chunk entries.
/// Each entry is either a chunk index, PROXY_FLAG | chunk_index, or 0 (empty).
pub struct Grid {
	pub dims: [u32; 3],
	pub entries: Vec<u32>,
	/// World-space origin of the grid (min corner).
	pub origin: [f32; 3],
	/// Side length of one chunk in meters.
	pub chunk_size: f32,
}

impl Grid {
	pub fn new(dims: [u32; 3], origin: [f32; 3], chunk_size: f32) -> Self {
		todo!()
	}

	pub fn get(&self, x: u32, y: u32, z: u32) -> u32 {
		todo!()
	}

	pub fn set(&mut self, x: u32, y: u32, z: u32, value: u32) {
		todo!()
	}
}
