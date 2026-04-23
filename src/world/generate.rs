use crate::chunk::Chunk;
use super::World;

impl World {
	// Generate a chunk from scratch by filtering the shape edit list to this chunk's
	// AABB, then running the coverage walk. lod determines voxel scale.
	pub fn generate_chunk(&self, chunk_pos: [i64; 3], lod: u8) -> Chunk { todo!() }
}
