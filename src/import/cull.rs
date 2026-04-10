use crate::import::VoxelSample;
use crate::import::voxelize::{morton_decode, morton_encode};
use std::collections::HashSet;

/// Remove any voxel whose all 6 face-neighbors are occupied and non-transparent.
/// These are never visible from any direction.
pub fn cull_interior(samples: &mut Vec<VoxelSample>) {
  if samples.is_empty() { return; }

  let occupied: HashSet<u64> = samples.iter().map(|s| s.morton).collect();

  let neighbor_offsets: [[i64; 3]; 6] = [
    [1, 0, 0], [-1, 0, 0],
    [0, 1, 0], [0, -1, 0],
    [0, 0, 1], [0, 0, -1],
  ];

  samples.retain(|s| {
    // Transparent voxels are never culled (they affect light)
    if s.voxel.transparent() { return true; }

    let [x, y, z] = morton_decode(s.morton).map(|v| v as i64);

    for [dx, dy, dz] in neighbor_offsets {
      let nx = x + dx;
      let ny = y + dy;
      let nz = z + dz;

      // If a neighbor coord goes negative, it's outside the chunk — keep this voxel
      if nx < 0 || ny < 0 || nz < 0 { return true; }

      let neighbor_code = morton_encode(nx as u32, ny as u32, nz as u32);
      if !occupied.contains(&neighbor_code) { return true; }
    }

    false // all 6 neighbors present and occupied — interior, cull it
  });
}
