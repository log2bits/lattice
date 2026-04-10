use crate::voxel::Voxel;

pub struct PbrSample {
	pub rgb: [u8; 3],
	pub roughness: f32,
	pub metallic: f32,
	pub emissive: bool,
}

/// Convert PBR material properties to the Voxel bit layout.
pub fn pbr_to_voxel(sample: &PbrSample) -> Voxel {
	todo!()
}
