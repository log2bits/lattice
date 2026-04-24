use crate::types::Voxel;

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
	pub fn get_or_add(&mut self, voxel: Voxel) -> u32 {
		todo!()
	}
	// Most frequently occurring index in the iterator, used for LOD values.
	pub fn mode(indices: impl Iterator<Item = u32>) -> u32 {
		todo!()
	}
}
