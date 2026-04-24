mod generate;
mod lod;
mod pool;

pub use lod::PointOfInterest;
pub use pool::ChunkPool;

use crate::{
	chunk::{Chunk, VoxelEdit},
	shape::Shape,
	tree::{Aabb, Ray, RayHit, Tree},
	types::Voxel,
};
use std::collections::HashMap;
use std::time::Duration;

pub const WORLD_DEPTH: u8 = 28;

pub struct World {
	pub world_tree: Tree,
	pub pool: ChunkPool,
	pub shape_edits: Vec<ShapeEdit>,
	// Chunks that have received player voxel edits, stored permanently.
	// Keyed on LOD-0 chunk coordinates.
	pub persistent_chunks: HashMap<[i64; 3], Chunk>,
	// Chunks with pending voxel edits waiting to be flushed.
	// Sorted closest-camera-first at the start of each flush_edits call.
	rebuild_pending: Vec<[i64; 3]>,
}

pub struct ShapeEdit {
	pub aabb: Aabb, // cached from shape.aabb() for O(1) per-chunk rejection
	pub min_lod: u8,
	pub shape: Box<dyn Shape>,
}

pub struct WorldHit {
	pub chunk_pos: [i64; 3],
	pub local_pos: [u8; 3],
	pub normal: [i32; 3],
	pub voxel: Voxel,
}

impl World {
	pub fn new() -> Self {
		todo!()
	}
	pub fn add_shape_edit(&mut self, edit: ShapeEdit) {
		todo!()
	}
	// Queue a voxel edit for later flushing. Pushes the chunk onto rebuild_pending
	// if it isn't already there.
	pub fn queue_voxel_edit(&mut self, chunk_pos: [i64; 3], edit: VoxelEdit) {
		todo!()
	}
	// Flush pending chunk rebuilds within the given time budget, processing
	// closest-to-camera chunks first. Call once per frame.
	pub fn flush_edits(&mut self, budget: Duration, camera_chunk: [i64; 3]) {
		todo!()
	}
	pub fn tick_lod(&mut self, interests: &[PointOfInterest]) {
		todo!()
	}
	// Trace a ray through the world tree to find a chunk handle, then through
	// that chunk's tree to find the exact voxel hit.
	pub fn trace_ray(&self, ray: &Ray) -> Option<WorldHit> {
		todo!()
	}
}
