use crate::{tree::Aabb, voxel::Voxel};
use super::{Coverage, Shape};

pub struct Sphere {
	pub center: [i64; 3],
	pub radius: i64,
	pub material: Voxel,
}

impl Shape for Sphere {
	fn aabb(&self) -> Aabb { todo!() }
	fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage { todo!() }
}
