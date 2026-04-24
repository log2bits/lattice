use super::Tree;
use crate::types::{BitpackedArray, Lut};

// Sentinel value meaning "remove whatever is at this location".
pub const DELETE: u32 = u32::MAX;

impl<const DEPTH: usize> Tree<DEPTH> {
	pub fn apply_edits(&mut self, edits: OrderedEdits<DEPTH>) {
		todo!()
	}

	pub fn add_edit(&mut self, edit: Edit<DEPTH>) {
		self.edits.add_edit(edit);
	}
}

pub struct Edit<const DEPTH: usize> {
	pub path: [u8; DEPTH],
	pub level: u8,
	pub value: u32,
}

impl<const DEPTH: usize> Edit<DEPTH> {
	// position: world units, aligned to leaf_unit * 4^level.
	// level: 0 = single leaf, n = subtree covering 4^n leaves per side.
	pub fn new(value: u32, position: [u64; 3], level: u8, leaf_unit: u64) -> Self {
		debug_assert!(level as usize <= DEPTH);
		let leaf = position.map(|p| p / leaf_unit);
		let mut path = [0u8; DEPTH];
		for d in 0..(DEPTH - level as usize) {
			let shift = 2 * (DEPTH - 1 - d);
			let [x, y, z] = leaf.map(|p| ((p >> shift) & 3) as u8);
			path[d] = x | (y << 2) | (z << 4);
		}
		Self { path, level, value }
	}
}

pub struct EditPacket<const DEPTH: usize> {
	pub paths: Vec<[u8; DEPTH]>,
	pub levels: Vec<u8>,
	// Distinct values referenced by this packet. DELETE (u32::MAX) = remove voxel.
	pub lut: Lut<u32>,
	// One bitpacked LUT index per edit, parallel to paths/levels. Bit width = min_bits(lut.len()).
	pub values: BitpackedArray,
	pub sorted: bool,
}

impl<const DEPTH: usize> EditPacket<DEPTH> {
	pub fn new_unsorted() -> Self {
		Self {
			paths: Vec::new(),
			levels: Vec::new(),
			lut: Lut::new(),
			values: BitpackedArray::new(),
			sorted: false,
		}
	}

	pub fn add_edit(&mut self, edit: Edit<DEPTH>) {
		if !self.sorted {
			for i in 0..self.levels.len() {
				if self.levels[i] == edit.level && self.paths[i] == edit.path {
					let lut_index = self.lut.get_or_add(edit.value);
					self.values.set(i as u32, lut_index);
					return;
				}
			}
		}

		self.paths.push(edit.path);
		self.levels.push(edit.level);
		self.values.push(self.lut.get_or_add(edit.value));
	}
}

#[derive(Default)]
pub struct OrderedEdits<const DEPTH: usize> {
	pub packets: Vec<EditPacket<DEPTH>>,
}

impl<const DEPTH: usize> OrderedEdits<DEPTH> {
	pub fn add_edit(&mut self, edit: Edit<DEPTH>) {
		let needs_new = self.packets.last().map_or(true, |p| p.sorted);
		if needs_new {
			self.packets.push(EditPacket::new_unsorted());
		}
		self.packets.last_mut().unwrap().add_edit(edit);
	}

	pub fn add_edit_packet(&mut self, packet: EditPacket<DEPTH>) {
		self.packets.push(packet);
	}
}
