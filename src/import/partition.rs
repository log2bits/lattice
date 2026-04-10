use crate::import::gltf::GltfMesh;

/// Map from flat chunk index to the triangle indices whose AABB overlaps that chunk.
pub struct PartitionMap {
	pub dims: [u32; 3],
	pub bins: Vec<Vec<u32>>,
}

/// One pass over all meshes. Each triangle goes into every chunk whose AABB it overlaps.
pub fn partition(meshes: &[GltfMesh], world_min: [f32; 3], voxel_size: f32, dims: [u32; 3], chunk_voxels: u32) -> PartitionMap {
	todo!()
}
