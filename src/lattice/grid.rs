use super::BitpackedArray;

// A flat 3D array of child entries. No tree structure, no deduplication. Each
// entry either points to a Chunk or carries LEAF_FLAG for proxy (unloaded) cells.
pub struct Grid {
	pub dims: [u32; 3],
	pub children: BitpackedArray,
}

impl Grid {
	pub fn new(dims: [u32; 3]) -> Self {
		let cell_count = dims[0] * dims[1] * dims[2];
		let mut children = BitpackedArray::new();
		for _ in 0..cell_count {
			children.push(0);
		}
		Self { dims, children }
	}

	// Inserts or overwrites a child entry at the given grid position.
	pub fn insert(&mut self, pos: [u32; 3], value: u32) {
		let idx = pos[2] * self.dims[0] * self.dims[1] + pos[1] * self.dims[0] + pos[0];
		self.children.set(idx, value)
	}

	pub fn get(&self, pos: [u32; 3]) -> u32 {
		let idx = pos[2] * self.dims[0] * self.dims[1] + pos[1] * self.dims[0] + pos[0];
		self.children.get(idx)
	}
}
