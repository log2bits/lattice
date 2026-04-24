use super::World;

pub struct PointOfInterest {
	pub world_pos: [i64; 3],
	// Deepest tree level to resolve for this point. Camera = WORLD_DEPTH (full LOD-0).
	pub max_depth: u8,
}

impl World {
	// Merge 64 LOD-(k-1) child chunks into one LOD-k chunk. Frees the 64 pool
	// slots and returns the handle of the new coarser chunk.
	pub fn coarsen_chunk(&mut self, child_handles: [u32; 64]) -> u32 {
		todo!()
	}

	// Split one LOD-k chunk into 64 LOD-(k-1) chunks. The parent slot is freed.
	// Each child is initialized from the parent and marked for shape resolution.
	// Returns the 64 new handles.
	pub fn split_chunk(&mut self, parent_handle: u32) -> [u32; 64] {
		todo!()
	}
}
