mod apply;
mod sort;

use crate::types::{BitpackedArray, Lut};

pub const DELETE: u32 = u32::MAX;

// Each byte is 1..=64 (raw child index + 1). Trailing 0s encode the level.
// level() = 0 means leaf (all DEPTH slots filled), DEPTH means root (all zeros).
// Lexicographic order = preorder traversal order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TreePath<const DEPTH: usize>([u8; DEPTH]);

impl<const DEPTH: usize> TreePath<DEPTH> {
	// level: 0 = single leaf, DEPTH = root.
	pub fn new(position: [u64; 3], level: u8, leaf_unit: u64) -> Self {
		debug_assert!(level as usize <= DEPTH);
		let depth = DEPTH - level as usize;
		let leaf = position.map(|p| p / leaf_unit);
		let mut path = [0u8; DEPTH];
		for d in 0..depth {
			let shift = 2 * (DEPTH - 1 - d);
			let [x, y, z] = leaf.map(|p| ((p >> shift) & 3) as u8);
			path[d] = (x | (y << 2) | (z << 4)) + 1;
		}
		Self(path)
	}

	pub fn from_packed(path: [u8; DEPTH]) -> Self {
		Self(path)
	}

	pub fn from_raw(indices: [u8; DEPTH], level: u8) -> Self {
		debug_assert!(level as usize <= DEPTH);
		let depth = DEPTH - level as usize;
		let mut path = [0u8; DEPTH];
		for i in 0..depth {
			path[i] = indices[i] + 1;
		}
		Self(path)
	}

	pub fn level(&self) -> u8 {
		let depth = self.0.iter().position(|&b| b == 0).unwrap_or(DEPTH);
		(DEPTH - depth) as u8
	}

	pub fn as_bytes(&self) -> &[u8; DEPTH] {
		&self.0
	}

	pub fn to_raw(&self) -> ([u8; DEPTH], u8) {
		let level = self.level();
		let depth = DEPTH - level as usize;
		let mut out = [0u8; DEPTH];
		for i in 0..depth {
			out[i] = self.0[i] - 1;
		}
		(out, level)
	}
}

pub struct Edit<const DEPTH: usize> {
	pub path: TreePath<DEPTH>,
	pub value: u32,
}

impl<const DEPTH: usize> Edit<DEPTH> {
	pub fn new(value: u32, position: [u64; 3], level: u8, leaf_unit: u64) -> Self {
		Self { path: TreePath::new(position, level, leaf_unit), value }
	}
}

#[derive(Clone)]
pub struct EditPacket<const DEPTH: usize> {
	pub paths: Vec<TreePath<DEPTH>>,
	// Distinct values referenced by this packet. DELETE (u32::MAX) = remove voxel.
	pub lut: Lut<u32>,
	// One bitpacked LUT index per edit, parallel to paths. Bit width = min_bits(lut.len()).
	pub values: BitpackedArray,
	pub sorted: bool,
}

impl<const DEPTH: usize> EditPacket<DEPTH> {
	pub fn new(sorted: bool) -> Self {
		Self {
			paths: Vec::new(),
			lut: Lut::new(),
			values: BitpackedArray::new(),
			sorted,
		}
	}

	pub fn add_edit(&mut self, edit: Edit<DEPTH>) {
		if !self.sorted {
			for i in 0..self.paths.len() {
				if self.paths[i] == edit.path {
					let lut_index = self.lut.get_or_add(edit.value);
					self.values.set(i as u32, lut_index);
					return;
				}
			}
		}

		self.paths.push(edit.path);
		self.values.push(self.lut.get_or_add(edit.value));
	}

}

#[derive(Default, Clone)]
pub struct OrderedEdits<const DEPTH: usize> {
	pub packets: Vec<EditPacket<DEPTH>>,
}

impl<const DEPTH: usize> OrderedEdits<DEPTH> {
	pub fn add_edit(&mut self, edit: Edit<DEPTH>) {
		let needs_new = self.packets.last().map_or(true, |p| p.sorted);
		if needs_new {
			self.packets.push(EditPacket::new(false));
		}
		self.packets.last_mut().unwrap().add_edit(edit);
	}

	pub fn add_edit_packet(&mut self, packet: EditPacket<DEPTH>) {
		self.packets.push(packet);
	}
}
