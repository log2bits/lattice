mod rect;
mod sphere;
mod terrain;

pub use rect::Rect;
pub use sphere::Sphere;
pub use terrain::Terrain;

use crate::{tree::Aabb, types::Voxel};

pub enum Coverage {
	Full(Voxel),
	Partial,
	Empty,
}

pub trait Shape: Send + Sync {
	fn aabb(&self) -> Aabb;
	// Given a node's world-space AABB and its depth level, classify coverage.
	// At leaf level the AABB covers exactly one voxel.
	fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage;
}
