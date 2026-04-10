pub mod chunk;
pub mod grid;
pub mod node;
pub mod pool;
pub mod walk;

use crate::tree::chunk::Chunk;
use crate::tree::grid::Grid;
use crate::tree::pool::NodePool;

/// The full voxel world: a 3D grid of chunks, each chunk a sparse 64-tree.
pub struct Lattice {
	pub depth: u8,
	pub voxel_size: f32,
	pub grid: Grid,
	pub chunks: Vec<Chunk>,
	/// NodePools for each depth level, shared across all chunks (SoA per depth).
	pub pools: Vec<NodePool>,
}
