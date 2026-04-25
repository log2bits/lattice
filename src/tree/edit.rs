use super::Tree;
use crate::types::{BitpackedArray, Lut};

// Sentinel value meaning "remove whatever is at this location".
pub const DELETE: u32 = u32::MAX;

impl<const DEPTH: usize> Tree<DEPTH> {
	pub fn apply_ordered_edits(&mut self, edits: OrderedEdits<DEPTH>) {
		edits.packets
			.into_iter()
			.for_each(|packet| self.apply_edit_packet(packet));
	}

	pub fn apply_edit_packet(&mut self, mut packet: EditPacket<DEPTH>) {
		packet.sort();
		todo!();
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
	pub fn new(sorted: bool) -> Self {
		Self {
			paths: Vec::new(),
			levels: Vec::new(),
			lut: Lut::new(),
			values: BitpackedArray::new(),
			sorted,
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

	pub fn sort(&mut self) {
        if self.sorted || self.paths.is_empty() {
            self.sorted = true;
            return;
        }

        let len = self.paths.len();

        // --- unpack values (critical for performance) ---
        let mut values = Vec::with_capacity(len);
        for i in 0..len {
            values.push(self.values.get(i as u32));
        }

        // --- index permutation ---
        let mut idx: Vec<u32> = (0..len as u32).collect();
        let mut tmp = vec![0u32; len];

        // --- radix sort by path (LSB → MSB) ---
        let mut counts = [0usize; 64];

        for d in (0..DEPTH).rev() {
            counts.fill(0);

            // histogram
            for &i in &idx {
                counts[self.paths[i as usize][d] as usize] += 1;
            }

            // prefix sum
            let mut sum = 0;
            for c in &mut counts {
                let n = *c;
                *c = sum;
                sum += n;
            }

            // scatter
            for &i in &idx {
                let key = self.paths[i as usize][d] as usize;
                tmp[counts[key]] = i;
                counts[key] += 1;
            }

            std::mem::swap(&mut idx, &mut tmp);
        }

        // --- stable level sort (descending) ---
        let mut counts = [0usize; 256];

        for &i in &idx {
            counts[self.levels[i as usize] as usize] += 1;
        }

        let mut sum = 0;
        for l in (0..256).rev() {
            let n = counts[l];
            counts[l] = sum;
            sum += n;
        }

        for &i in &idx {
            let l = self.levels[i as usize] as usize;
            tmp[counts[l]] = i;
            counts[l] += 1;
        }

        std::mem::swap(&mut idx, &mut tmp);

        // --- reorder + deduplicate in one pass ---
        let mut new_paths = Vec::with_capacity(len);
        let mut new_levels = Vec::with_capacity(len);
        let mut new_values = Vec::with_capacity(len);

        for &i in &idx {
            let path = self.paths[i as usize];
            let level = self.levels[i as usize];
            let value = values[i as usize];

            if let Some(last) = new_paths.len().checked_sub(1) {
                if new_paths[last] == path && new_levels[last] == level {
                    // last write wins
                    new_values[last] = value;
                    continue;
                }
            }

            new_paths.push(path);
            new_levels.push(level);
            new_values.push(value);
        }

        // --- repack ---
        self.paths = new_paths;
        self.levels = new_levels;

        self.values.clear();
        for v in new_values {
            self.values.push(v);
        }

        self.sorted = true;
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
			self.packets.push(EditPacket::new(false));
		}
		self.packets.last_mut().unwrap().add_edit(edit);
	}

	pub fn add_edit_packet(&mut self, packet: EditPacket<DEPTH>) {
		self.packets.push(packet);
	}
}
