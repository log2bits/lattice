use crate::import::gltf::{GltfMaterial, GltfMesh};
use crate::import::palette::Palette;
use crate::import::VoxelSample;

/// Rasterize triangles for one chunk into morton-sorted VoxelSamples.
///
/// Uses barycentric projection onto the dominant axis with fat voxelization
/// (guarantees 6-connected surfaces so interior culling doesn't punch holes).
pub fn voxelize_chunk(meshes: &[GltfMesh], materials: &[GltfMaterial], triangle_indices: &[u32], chunk_origin: [f32; 3], voxel_size: f32, chunk_voxels: u32, palette: &Palette) -> Vec<VoxelSample> {
	todo!()
}
