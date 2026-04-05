use crate::lattice::{ColorPalette, Voxel};

// A PBR material sampled at a surface point, ready to be converted to a Voxel.
pub struct SampledMaterial {
	pub base_color: [f32; 4],
	pub roughness: f32,
	pub metallic: f32,
	pub emissive: [f32; 3],
}

// Maps a sampled PBR material to a Voxel by quantizing the base color to the
// nearest palette entry and encoding the material flags.
pub fn material_to_voxel(mat: &SampledMaterial, palette: &ColorPalette) -> Voxel {
	todo!()
}
