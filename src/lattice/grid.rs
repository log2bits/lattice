use super::BitpackedArray;

// A flat 3D array of child entries. No tree structure, no deduplication. Each
// entry either points to a sub-DAG root in the next section or is empty.
//
// Grid is right for the top of the hierarchy where deduplication almost never
// fires. It avoids tree traversal overhead in regions that are nearly all unique.
pub struct GridLevel {
	pub dims: [u32; 3],
	pub children: BitpackedArray,
}

impl GridLevel {
	pub fn new(dims: [u32; 3]) -> Self {
		let cell_count = dims[0] * dims[1] * dims[2];
		let mut children = BitpackedArray::new();
		for _ in 0..cell_count {
			children.push(0);
		}
		Self { dims, children }
	}

	// Inserts or overwrites a child entry at the given local grid position.
	// Can be called in any order before finalize().
	pub fn insert(&mut self, pos: [u32; 3], value: u32) {
		let idx = pos[2] * self.dims[0] * self.dims[1] + pos[1] * self.dims[0] + pos[0];
		self.children.set(idx, value)
	}

	pub fn get(&self, pos: [u32; 3]) -> u32 {
		let idx = pos[2] * self.dims[0] * self.dims[1] + pos[1] * self.dims[0] + pos[0];
		self.children.get(idx)
	}
}
