use crate::chunk::Chunk;

pub struct ChunkPool {
	chunks: Vec<Option<Chunk>>,
	dirty: Vec<bool>,
	free: Vec<u32>,
}

impl ChunkPool {
	pub fn new() -> Self { todo!() }
	pub fn alloc(&mut self, chunk: Chunk) -> u32 { todo!() }
	pub fn free(&mut self, handle: u32) { todo!() }
	pub fn get(&self, handle: u32) -> Option<&Chunk> { todo!() }
	pub fn get_mut(&mut self, handle: u32) -> Option<&mut Chunk> { todo!() }
	pub fn mark_dirty(&mut self, handle: u32) { todo!() }
	// Handles that need re-upload to GPU. Clears the dirty flags.
	pub fn take_dirty(&mut self) -> Vec<u32> { todo!() }
}
