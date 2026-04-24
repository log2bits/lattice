use super::{Coverage, Shape};
use crate::{tree::Aabb, types::Voxel};

pub struct Rect {
	pub min: [i64; 3],
	pub max: [i64; 3],
	pub material: Voxel,
}

impl Shape for Rect {
	fn aabb(&self) -> Aabb {
		todo!()
	}
	fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage {
		todo!()
	}
}
