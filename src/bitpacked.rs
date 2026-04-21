#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BitpackedArray {
	pub data: Vec<u32>,
	pub bits: u8,
	pub len: u32,
}

impl Default for BitpackedArray {
	fn default() -> Self {
		Self::new()
	}
}

impl BitpackedArray {
	pub fn new() -> Self {
		Self {
			data: Vec::new(),
			bits: 1,
			len: 0,
		}
	}

	pub fn len(&self) -> u32 {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	pub fn clear(&mut self) {
		self.data.clear();
		self.bits = 1;
		self.len = 0;
	}

	#[inline]
	pub fn push(&mut self, value: u32) {
		self.ensure_width(value);
		let bit_pos = self.len << self.bits.trailing_zeros();
		let bit_offset = bit_pos & 31;
		if bit_offset == 0 {
			self.data.push(0);
		}
		self.data[(bit_pos >> 5) as usize] |= value << bit_offset;
		self.len += 1;
	}

	#[inline]
	pub fn get(&self, index: u32) -> u32 {
		let bit_pos = index << self.bits.trailing_zeros();
		(self.data[(bit_pos >> 5) as usize] >> (bit_pos & 31)) & Self::mask(self.bits)
	}

	#[inline]
	pub fn set(&mut self, index: u32, value: u32) {
		self.ensure_width(value);
		let bit_pos = index << self.bits.trailing_zeros();
		let bit_off = bit_pos & 31;
		let mask = Self::mask(self.bits) << bit_off;
		self.data[(bit_pos >> 5) as usize] =
			(self.data[(bit_pos >> 5) as usize] & !mask) | (value << bit_off);
	}

	#[inline]
	fn ensure_width(&mut self, value: u32) {
		if self.bits < 32 && value >> self.bits != 0 {
			self.repack_in_place(((32 - value.leading_zeros()) as u8).next_power_of_two());
		}
	}

	pub fn repack_in_place(&mut self, new_bits: u8) {
		assert!(matches!(new_bits, 1 | 2 | 4 | 8 | 16 | 32));
		if new_bits == self.bits {
			return;
		}
		let (old_bits, len) = (self.bits, self.len);
		let new_word_count = ((len as usize * new_bits as usize) + 31) >> 5;
		let growing = new_bits > old_bits;
		if growing {
			self.data.resize(new_word_count, 0);
		}
		for step in 0..len {
			let entry = if growing { len - 1 - step } else { step };
			let old_pos = entry << old_bits.trailing_zeros();
			let value =
				(self.data[(old_pos >> 5) as usize] >> (old_pos & 31)) & Self::mask(old_bits);
			let new_pos = entry << new_bits.trailing_zeros();
			let new_off = new_pos & 31;
			let mask = Self::mask(new_bits) << new_off;
			self.data[(new_pos >> 5) as usize] =
				(self.data[(new_pos >> 5) as usize] & !mask) | (value << new_off);
		}
		if !growing {
			self.data.truncate(new_word_count);
		}
		self.bits = new_bits;
	}

	pub fn insert(&mut self, index: u32, value: u32) {
		assert!(index <= self.len);
		self.ensure_width(value);
		let tail = if self.len > 0 {
			self.get(self.len - 1)
		} else {
			0
		};
		self.push(tail);
		let mut i = self.len - 1;
		while i > index {
			self.set(i, self.get(i - 1));
			i -= 1;
		}
		self.set(index, value);
	}

	pub fn remove(&mut self, index: u32) -> u32 {
		assert!(index < self.len);
		let removed = self.get(index);
		for i in index..self.len - 1 {
			self.set(i, self.get(i + 1));
		}
		self.len -= 1;
		let new_word_count = ((self.len as usize * self.bits as usize) + 31) >> 5;
		self.data.truncate(new_word_count);
		removed
	}

	pub fn repack(&self, new_bits: u8) -> Self {
		let mut out = self.clone();
		out.repack_in_place(new_bits);
		out
	}

	pub fn min_bits(count: u32) -> u8 {
		let bits = (32 - count.saturating_sub(1).leading_zeros()).max(1) as u8;
		bits.next_power_of_two()
	}

	fn mask(bits: u8) -> u32 {
		if bits == 32 {
			u32::MAX
		} else {
			(1u32 << bits) - 1
		}
	}
}
