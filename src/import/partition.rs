use crate::import::gltf::GltfScene;
use glam::Vec3;

/// Map from flat chunk index to triangle indices whose AABB overlaps that chunk.
pub struct PartitionMap {
  pub dims: [u32; 3],
  /// bins[cz * dims[0]*dims[1] + cy * dims[0] + cx] = triangle indices
  pub bins: Vec<Vec<usize>>,
}

/// One pass over all triangles; each triangle is added to every chunk whose AABB it overlaps.
pub fn partition(
  scene: &GltfScene,
  world_min: [f32; 3],
  voxel_size: f32,
  dims: [u32; 3],
  chunk_voxels: u32,
) -> PartitionMap {
  let chunk_size_m = chunk_voxels as f32 * voxel_size;
  let total = (dims[0] * dims[1] * dims[2]) as usize;
  let mut bins: Vec<Vec<usize>> = vec![Vec::new(); total];

  for (tri_idx, tri) in scene.triangles.iter().enumerate() {
    let [a, b, c] = tri.positions_glam();

    // Triangle AABB in world space
    let tri_min = a.min(b).min(c);
    let tri_max = a.max(b).max(c);

    // Convert to chunk grid coords (with 1-voxel slop for fat triangles)
    let slop = voxel_size; // one voxel of slop
    let origin = Vec3::from_array(world_min);

    let min_chunk = ((tri_min - slop - origin) / chunk_size_m)
      .floor()
      .as_ivec3();
    let max_chunk = ((tri_max + slop - origin) / chunk_size_m)
      .floor()
      .as_ivec3();

    let cx0 = min_chunk.x.max(0) as u32;
    let cy0 = min_chunk.y.max(0) as u32;
    let cz0 = min_chunk.z.max(0) as u32;
    let cx1 = (max_chunk.x as u32 + 1).min(dims[0]);
    let cy1 = (max_chunk.y as u32 + 1).min(dims[1]);
    let cz1 = (max_chunk.z as u32 + 1).min(dims[2]);

    for cz in cz0..cz1 {
      for cy in cy0..cy1 {
        for cx in cx0..cx1 {
          let flat = (cz * dims[1] + cy) * dims[0] + cx;
          bins[flat as usize].push(tri_idx);
        }
      }
    }
  }

  PartitionMap { dims, bins }
}
