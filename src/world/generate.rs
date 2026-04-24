use super::World;
use crate::chunk::Chunk;

impl World {
	// Generate a chunk at the given LOD by filtering shape_edits to this chunk's
	// AABB and running the coverage walk. If any persistent chunks intersect the
	// AABB, their data is aggregated into the result at the appropriate level:
	// LOD-0 copies directly, coarser LODs aggregate upward from the persistent
	// chunk trees.
	pub fn generate_chunk(&self, chunk_pos: [i64; 3], lod: u8) -> Chunk {
		todo!()
	}
}
