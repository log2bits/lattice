use std::collections::HashMap;

// Per-chunk palette. Maps unique Voxel values to compact indices.
// The bit width of any index array referencing this LUT is determined by len().
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
