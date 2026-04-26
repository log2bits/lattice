use super::{Coverage, Shape};
use crate::tree::Aabb;

pub struct Terrain {
	pub seed: u64,
}

impl Shape for Terrain {
	fn aabb(&self) -> Aabb {
		todo!()
	}
	fn coverage(&self, node_aabb: Aabb, lod: u8) -> Coverage {
		todo!()
	}
}
