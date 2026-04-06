use std::collections::HashMap;

use super::{BitpackedArray, Voxel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lut {
	pub values: Vec<u32>,
	lookup: HashMap<u32, u32>,
}

impl Lut {
	pub fn new() -> Self {
		Self {
			values: Vec::new(),
			lookup: HashMap::new(),
		}
	}

	pub fn with_capacity(cap: u32) -> Self {
		Self {
			values: Vec::with_capacity(cap as usize),
			lookup: HashMap::with_capacity(cap as usize),
		}
	}

	// Returns the index of value in the table, inserting it if not present.
	pub fn insert(&mut self, value: u32) -> u32 {
		if let Some(&idx) = self.lookup.get(&value) {
			return idx;
		}
		let idx = self.values.len() as u32;
		self.values.push(value);
		self.lookup.insert(value, idx);
		idx
	}

	// Returns the index of value if it is already in the table.
	pub fn get(&self, value: u32) -> Option<u32> {
		self.lookup.get(&value).copied()
	}

	pub fn len(&self) -> u32 {
		self.values.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}
}

impl Default for Lut {
	fn default() -> Self {
		Self::new()
	}
}

// Per-root material storage. The LUT holds unique Voxel values referenced by
// the bitpacked index array. The bit width of indices is determined by the LUT
// size and is stored in indices.bits. Both live together because they are
// always owned and used as a unit by a single GeometryDagRoot.
pub struct MaterialsArray {
	pub lut: Vec<Voxel>,
	pub indices: BitpackedArray,
	lookup: HashMap<u32, u32>, // build-time only: voxel raw -> lut index
}

impl MaterialsArray {
	pub fn new() -> Self {
		Self {
			lut: Vec::new(),
			indices: BitpackedArray::new(),
			lookup: HashMap::new(),
		}
	}

	// Pushes a voxel into the array, deduplicating into the LUT.
	pub fn push(&mut self, voxel: Voxel) {
		let raw = voxel.into();
		let idx = if let Some(&idx) = self.lookup.get(&raw) {
			idx
		} else {
			let idx = self.lut.len() as u32;
			self.lut.push(voxel);
			self.lookup.insert(raw, idx);
			idx
		};
		self.indices.push(idx);
	}

	// Returns the voxel at position i in the Dolonius DFS order.
	pub fn get(&self, i: u32) -> Voxel {
		self.lut[self.indices.get(i) as usize]
	}

	pub fn len(&self) -> u32 {
		self.indices.len()
	}

	pub fn is_empty(&self) -> bool {
		self.indices.is_empty()
	}
}

impl Default for MaterialsArray {
	fn default() -> Self {
		Self::new()
	}
}
