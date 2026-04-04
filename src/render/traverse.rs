#![allow(unused)]

// CPU-side reference implementation of the 64-tree DDA traversal.
// The actual render path runs in traverse.wgsl; this is for testing and debugging.

pub struct HitResult {
  pub t:          f32,       // ray parameter at hit
  pub position:   [f32; 3],
  pub face_normal:[f32; 3],  // derived from DDA exit face, not stored per-voxel
  pub voxel:      u32,       // decoded 32-bit voxel value from global voxel LUT
}

// Traces a ray through the lattice and returns the first hit, if any.
pub fn trace_ray(
  origin:    [f32; 3],
  direction: [f32; 3],
  // lattice data would be passed here once types are finalized
) -> Option<HitResult> {
  todo!()
}
