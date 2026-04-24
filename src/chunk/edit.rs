use crate::types::Voxel;

pub struct VoxelEdit {
	pub pos: [u8; 3],         // chunk-local: each component in [0, 255]
	pub voxel: Option<Voxel>, // None = remove
}
