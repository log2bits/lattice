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
		Self {
			dims,
			children: BitpackedArray::new(),
		}
	}

	pub fn cell_count(&self) -> u32 {
		self.dims[0] * self.dims[1] * self.dims[2]
	}

	pub fn is_empty(&self) -> bool {
		self.dims[0] == 0 || self.dims[1] == 0 || self.dims[2] == 0
	}

	fn flat_index(&self, pos: [u32; 3]) -> u32 {
		pos[2] * self.dims[0] * self.dims[1] + pos[1] * self.dims[0] + pos[0]
	}

	pub fn get(&self, pos: [u32; 3]) -> u32 {
		self.children.get(self.flat_index(pos))
	}

	pub fn set(&mut self, pos: [u32; 3], value: u32) {
		self.children.set(self.flat_index(pos), value);
	}
}
