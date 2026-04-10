use crate::tree::Lattice;

/// Result of walking down to a leaf.
pub struct WalkResult {
	/// Slot index at each depth level (length == lattice.depth).
	pub path: Vec<u8>,
	/// MaterialTable index at the leaf.
	pub material_idx: u32,
}

/// Walk down the tree to the voxel at world-space position within a chunk.
/// Returns None if the position falls outside the chunk or is empty.
pub fn walk_down(lattice: &Lattice, chunk_idx: u32, pos: [f32; 3]) -> Option<WalkResult> {
	todo!()
}

/// Walk back up from a leaf path, recomputing lod_material at each level.
pub fn walk_up(lattice: &mut Lattice, chunk_idx: u32, path: &[u8]) {
	todo!()
}
