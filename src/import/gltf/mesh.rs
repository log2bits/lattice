// A triangle with a material index, ready for voxelization.
pub struct Triangle {
  pub verts:        [[f32; 3]; 3],
  pub uvs:          [[f32; 2]; 3],
  pub material_idx: usize,
}

// Extracts triangles from a glTF primitive.
pub fn extract_triangles(
  primitive: &gltf::Primitive,
  buffers:   &[gltf::buffer::Data],
) -> Vec<Triangle> {
  todo!()
}

// Clips a triangle to the AABB of a voxel chunk and returns the sub-triangles.
pub fn clip_to_chunk(tri: &Triangle, chunk_min: [f32; 3], chunk_max: [f32; 3]) -> Vec<Triangle> {
  todo!()
}
