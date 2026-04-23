use crate::{tree::Aabb, voxel::Voxel};
use super::{Coverage, Shape};

pub struct Rect {
	pub min: [i64; 3],
	pub max: [i64; 3],
	pub material: Voxel,
}

impl Shape for Rect {
	fn aabb(&self) -> Aabb { todo!() }
	fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage { todo!() }
}
