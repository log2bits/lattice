use crate::tree::Aabb;
use super::{Coverage, Shape};

pub struct Terrain {
	pub seed: u64,
}

impl Shape for Terrain {
	fn aabb(&self) -> Aabb { todo!() }
	fn coverage(&self, node_aabb: Aabb, level: u8) -> Coverage { todo!() }
}
