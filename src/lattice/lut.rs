use std::collections::HashMap;

// A set of unique u32 values, referenced by index. Callers store indices in
// a BitpackedArray, which auto-repacks as values are pushed into it.
pub struct Lut {
	pub values: Vec<u32>,
	dedup: HashMap<u32, u32>,
}

impl Lut {
	pub fn new() -> Self {
		Self {
			values: Vec::new(),
			dedup: HashMap::new(),
		}
	}

	pub fn with_capacity(cap: u32) -> Self {
		Self {
			values: Vec::with_capacity(cap as usize),
			dedup: HashMap::with_capacity(cap as usize),
		}
	}

	// Returns the index of value in the table, inserting it if not present.
	pub fn insert(&mut self, value: u32) -> u32 {
		if let Some(&idx) = self.dedup.get(&value) {
			return idx;
		}
		let idx = self.values.len() as u32;
		self.values.push(value);
		self.dedup.insert(value, idx);
		idx
	}

	// Returns the index of value if it is already in the table.
	pub fn get(&self, value: u32) -> Option<u32> {
		self.dedup.get(&value).copied()
	}

	pub fn len(&self) -> u32 {
		self.values.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}
}
