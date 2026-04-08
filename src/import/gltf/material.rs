use crate::import::color;
use crate::lattice::Voxel;

// A PBR material sampled at a surface point, ready to be converted to a Voxel.
pub struct SampledMaterial {
	pub base_color: [f32; 4],
	pub roughness: f32,
	pub metallic: f32,
	pub emissive: [f32; 3],
}

// A glTF material with its textures decoded and ready for per-voxel sampling.
// Textures are stored as flat RGB pixel arrays with their widths.
pub struct PreparedMaterial {
	pub base_color_factor: [f32; 4],
	pub roughness_factor: f32,
	pub metallic_factor: f32,
	pub emissive_factor: [f32; 3],
	// base_color_texture pixels (linear RGB) and width, or None if no texture.
	pub base_color_texture: Option<(Vec<[u8; 3]>, u32)>,
}

// Decodes all glTF materials into PreparedMaterials, pulling pixel data from images.
pub fn prepare_materials(document: &gltf::Document, images: &[gltf::image::Data]) -> Vec<PreparedMaterial> {
	todo!()
}

// Maps a sampled PBR material to a Voxel by quantizing the base color to the
// nearest palette entry and encoding the material flags.
pub fn material_to_voxel(mat: &SampledMaterial, palette: &color::Palette) -> Voxel {
	todo!()
}
