use std::collections::HashMap;
use std::hash::Hash;
use super::bitpacked::BitpackedArray;

// A set of unique values of type T, referenced by bitpacked indices.
// Used for the global voxel LUT and per-section-root LUTs.
//
// During construction, call insert() to add values and get back local indices.
// Call finalize() once all values are added: it computes the minimum bit width
// and stores it in bits. The caller is responsible for repacking index storage.
pub struct Lut<T: Hash + Eq + Clone> {
  pub values: Vec<T>,
  pub bits:   u8,
  dedup:      HashMap<T, u32>,
}

impl<T: Hash + Eq + Clone> Lut<T> {
  pub fn new() -> Self {
    Self { values: Vec::new(), bits: 1, dedup: HashMap::new() }
  }

  pub fn with_capacity(cap: usize) -> Self {
    Self { values: Vec::with_capacity(cap), bits: 1, dedup: HashMap::with_capacity(cap) }
  }

  // Returns the index of value in the table, inserting it if not present.
  pub fn insert(&mut self, value: T) -> u32 {
    todo!()
  }

  // Returns the index of value if it is already in the table.
  pub fn get(&self, value: &T) -> Option<u32> {
    todo!()
  }

  pub fn len(&self) -> usize {
    self.values.len()
  }

  pub fn is_empty(&self) -> bool {
    self.values.is_empty()
  }

  // Computes the minimum bit width for the current table size and stores it in bits.
  pub fn finalize(&mut self) {
    self.bits = BitpackedArray::min_bits(self.values.len());
  }
}
