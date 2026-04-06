use std::collections::HashMap;

use super::BitpackedArray;

// A flat 3D array of child entries. No tree structure, no deduplication. Each
// entry either points to a sub-DAG root in the next section or is empty.
//
// Grid is right for the top of the hierarchy where deduplication almost never
// fires. It avoids tree traversal overhead in regions that are nearly all unique.
//
// During construction, entries are collected into a sparse HashMap without
// needing to know the bounds upfront. Call finalize() once all entries are
// inserted to compute the tight bounding dims and pack into the flat
// BitpackedArray. After finalization, the build-time map is dropped.
pub struct GridLevel {
	pub dims: [u32; 3],
	pub children: BitpackedArray,
	pub(crate) staging: HashMap<[u32; 3], u32>, // build-time only
}

impl GridLevel {
	pub fn new() -> Self {
		Self {
			dims: [0; 3],
			children: BitpackedArray::new(),
			staging: HashMap::new(),
		}
	}

	// Inserts or overwrites a child entry at the given local grid position.
	// Can be called in any order before finalize().
	pub fn insert(&mut self, pos: [u32; 3], value: u32) {
		self.staging.insert(pos, value);
	}

	// Computes the tight bounding dims from all inserted positions, packs
	// everything into the flat BitpackedArray, and clears the staging map.
	// Must be called before the grid is used for traversal or serialization.
	pub fn finalize(&mut self) {
		if self.staging.is_empty() {
			return;
		}
		let mut max = [0u32; 3];
		for pos in self.staging.keys() {
			max[0] = max[0].max(pos[0] + 1);
			max[1] = max[1].max(pos[1] + 1);
			max[2] = max[2].max(pos[2] + 1);
		}
		self.dims = max;
		let cell_count = max[0] * max[1] * max[2];
		self.children = BitpackedArray::new();
		for _ in 0..cell_count {
			self.children.push(0);
		}
		for (&pos, &value) in &self.staging {
			let idx = pos[2] * max[0] * max[1] + pos[1] * max[0] + pos[0];
			self.children.set(idx, value);
		}
		self.staging.clear();
	}

	pub fn cell_count(&self) -> u32 {
		self.dims[0] * self.dims[1] * self.dims[2]
	}

	pub fn is_empty(&self) -> bool {
		self.staging.is_empty() && self.dims[0] == 0
	}

	pub fn get(&self, pos: [u32; 3]) -> u32 {
		let idx = pos[2] * self.dims[0] * self.dims[1] + pos[1] * self.dims[0] + pos[0];
		self.children.get(idx)
	}
}

impl Default for GridLevel {
	fn default() -> Self {
		Self::new()
	}
}
