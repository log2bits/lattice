mod build;
mod edit;
mod material;

pub use edit::VoxelEdit;
pub use material::MaterialTable;

use crate::{tree::Tree, voxel::Voxel};

pub const DEPTH: u8 = 4;
pub const SIDE: u32 = 256; // 4^DEPTH

pub struct Chunk {
	pub tree: Tree,
	pub materials: MaterialTable,
	pending_edits: Vec<VoxelEdit>,
}

impl Chunk {
	pub fn new() -> Self { todo!() }
	pub fn memory_bytes(&self) -> usize { todo!() }
	// pos is chunk-local: each component in [0, 255]
	pub fn get_voxel(&self, pos: [u8; 3]) -> Option<Voxel> { todo!() }
	pub fn has_pending_edits(&self) -> bool { !self.pending_edits.is_empty() }
	pub fn queue_edit(&mut self, edit: VoxelEdit) { todo!() }
	pub fn flush_edits(&mut self) { todo!() }
}
