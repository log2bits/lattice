mod edit;

pub use edit::VoxelEdit;

use crate::{
	tree::{EditPacket, OrderedEdits, Tree},
	types::{Lut, Voxel},
};

pub const DEPTH: usize = 4;
pub const SIDE: u32 = 256; // 4^DEPTH

pub struct Chunk {
	pub tree: Tree<DEPTH>,
	pub materials: Lut<Voxel>,
}

impl Chunk {
	pub fn new() -> Self {
		todo!()
	}
	pub fn memory_bytes(&self) -> usize {
		todo!()
	}
	// pos is chunk-local: each component in [0, 255]
	pub fn get_voxel(&self, pos: [u8; 3]) -> Option<Voxel> {
		todo!()
	}
	pub fn has_pending_edits(&self) -> bool {
		!self.tree.edits.packets.is_empty()
	}
	// Append a player voxel edit. Adds to the last unsorted packet, or starts a
	// new one if the last packet is sorted.
	pub fn queue_edit(&mut self, edit: VoxelEdit) {
		todo!()
	}
	// Append a pre-sorted packet of shape edits from the coverage walk.
	pub fn add_shape_packet(&mut self, packet: EditPacket<DEPTH>) {
		todo!()
	}
	// Apply all pending edits to the tree and compact.
	pub fn flush_edits(&mut self) {
		todo!()
	}
}
