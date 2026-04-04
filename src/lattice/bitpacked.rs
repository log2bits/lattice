// A flat array of fixed-width values packed at a power-of-two bit width.
// Bit widths: 1, 2, 4, 8, 16, 32. The GPU extracts any entry with a single
// shift and mask. Backing storage is Vec<u32> for GPU compatibility.
pub struct BitpackedArray {
  pub data: Vec<u32>,
  pub bits: u8,
  pub len:  usize,
}

impl BitpackedArray {
  pub fn new(bits: u8) -> Self {
    assert!(matches!(bits, 1 | 2 | 4 | 8 | 16 | 32));
    Self { data: Vec::new(), bits, len: 0 }
  }

  pub fn with_capacity(bits: u8, cap: usize) -> Self {
    assert!(matches!(bits, 1 | 2 | 4 | 8 | 16 | 32));
    let words = cap.div_ceil(32 / bits as usize);
    Self { data: Vec::with_capacity(words), bits, len: 0 }
  }

  pub fn len(&self) -> usize {
    self.len
  }

  pub fn is_empty(&self) -> bool {
    self.len == 0
  }

  pub fn push(&mut self, value: u32) {
    todo!()
  }

  pub fn get(&self, index: usize) -> u32 {
    todo!()
  }

  pub fn set(&mut self, index: usize, value: u32) {
    todo!()
  }

  // Returns a new BitpackedArray with the same values repacked at new_bits.
  pub fn repack(&self, new_bits: u8) -> Self {
    todo!()
  }

  // Minimum power-of-two bit width needed to represent `count` distinct indices.
  pub fn min_bits(count: usize) -> u8 {
    todo!()
  }
}
