use crate::import::gltf::mesh::Triangle;

// Tests whether a triangle intersects a voxel using the SAT method.
pub fn triangle_voxel_intersect(tri: &Triangle, voxel_min: [f32; 3], voxel_size: f32) -> bool {
	todo!()
}

// Samples the texture at the given UV coordinate and returns linear RGB.
pub fn sample_texture(pixels: &[[u8; 3]], width: u32, uv: [f32; 2]) -> [u8; 3] {
	todo!()
}
