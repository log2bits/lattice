use crate::import::color::Palette;
use crate::import::gltf::material::PreparedMaterial;
use crate::import::gltf::mesh::Triangle;
use crate::import::VoxelSample;

// Tests whether a triangle intersects a voxel using the SAT method.
pub fn triangle_voxel_intersect(tri: &Triangle, voxel_min: [f32; 3], voxel_size: f32) -> bool {
	todo!()
}

// Samples the texture at the given UV coordinate and returns linear RGB.
pub fn sample_texture(pixels: &[[u8; 3]], width: u32, uv: [f32; 2]) -> [u8; 3] {
	todo!()
}

// Iterates over all voxels in the triangle's AABB within [vmin, vmax), tests each
// with triangle_voxel_intersect, samples the material, and appends VoxelSamples.
pub fn voxelize_triangle(tri: &Triangle, mat: &PreparedMaterial, palette: &mut Palette, voxel_size: f64, vmin: [i64; 3], vmax: [i64; 3], out: &mut Vec<VoxelSample>) {
	println!("voxelizing triangle");
	todo!()
}
