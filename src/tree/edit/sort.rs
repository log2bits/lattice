use super::{EditPacket, TreePath};
use crate::types::BitpackedArray;

impl<const DEPTH: usize> EditPacket<DEPTH> {
	pub fn sort(&mut self) {
		let n = self.paths.len();
		if n < 2 {
			return;
		}

		// Zip paths with their lut indices, sort by packed key, then unzip back.
		// This avoids a separate permutation pass and rebuilds values via sequential push.
		let mut entries: Vec<(TreePath<DEPTH>, u32)> = self.paths.iter()
			.zip((0..n as u32).map(|i| self.values.get(i)))
			.map(|(&p, v)| (p, v))
			.collect();

		sort_entries::<DEPTH>(&mut entries);

		let mut new_values = BitpackedArray::with_bits(self.values.bits);
		for (i, (path, val)) in entries.into_iter().enumerate() {
			self.paths[i] = path;
			new_values.push(val);
		}
		self.values = new_values;
		self.sorted = true;
	}
}

// 7 bits per slot, MSB first → integer order = preorder.
macro_rules! pack_key {
	($ty:ty, $path:expr) => {{
		let mut key: $ty = 0;
		for &b in $path.0.iter() {
			key = (key << 7) | b as $ty;
		}
		key
	}};
}

fn sort_entries<const DEPTH: usize>(entries: &mut [(TreePath<DEPTH>, u32)]) {
	if DEPTH * 7 <= 8 {
		radsort::sort_by_key(entries, |(p, _)| pack_key!(u8, p));
	} else if DEPTH * 7 <= 16 {
		radsort::sort_by_key(entries, |(p, _)| pack_key!(u16, p));
	} else if DEPTH * 7 <= 32 {
		radsort::sort_by_key(entries, |(p, _)| pack_key!(u32, p));
	} else if DEPTH * 7 <= 64 {
		radsort::sort_by_key(entries, |(p, _)| pack_key!(u64, p));
	} else if DEPTH * 7 <= 128 {
		radsort::sort_by_key(entries, |(p, _)| pack_key!(u128, p));
	} else {
		// DEPTH > 18: TreePath derives Ord (lexicographic on bytes) which gives the same
		// preorder as the packed key for correctly terminated paths.
		entries.sort_unstable_by(|(a, _), (b, _)| a.cmp(b));
	}
}
