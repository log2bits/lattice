use crate::{tree::Level, voxel::Voxel};

pub struct Chunk {
	pub root: u32,
	pub materials: MaterialTable,
	pub levels: Vec<Level>,
}

pub struct MaterialTable {
	pub values: Vec<Voxel>,
}

impl Chunk {
	pub fn new() -> Self {
		todo!()
	}

	pub fn place_voxel(&self, pos: [u32; 3], voxel: Voxel) {
		todo!()
	}

	pub fn remove_voxel(&self, pos: [u32; 3]) {
		todo!()
	}

	pub fn get_voxel(&self, pos: [u32; 3]) -> Option<Voxel> {
		todo!()
	}
}
