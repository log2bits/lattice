use crate::voxel::Voxel;

#[derive(Default)]
pub struct MaterialTable {
	pub values: Vec<Voxel>,
}

impl MaterialTable {
	pub fn len(&self) -> u32 {
		self.values.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.values.is_empty()
	}

	pub fn get(&self, idx: u32) -> Voxel {
		self.values[idx as usize]
	}

	pub fn intern(&mut self, voxel: Voxel) -> u32 {
		if let Some(idx) = self.values.iter().position(|&v| v == voxel) {
			return idx as u32;
		}
		self.values.push(voxel);
		self.values.len() as u32 - 1
	}
}

/// Returns the most frequently occurring material index in the iterator.
pub(crate) fn material_mode(iter: impl Iterator<Item = u32>) -> u32 {
	let mut counts: [(u32, u32); 64] = [(0, 0); 64];
	let mut len = 0usize;
	'outer: for mat in iter {
		for entry in &mut counts[..len] {
			if entry.0 == mat {
				entry.1 += 1;
				continue 'outer;
			}
		}
		counts[len] = (mat, 1);
		len += 1;
	}
	counts[..len]
		.iter()
		.max_by_key(|&&(_, c)| c)
		.map(|&(m, _)| m)
		.unwrap_or(0)
}
