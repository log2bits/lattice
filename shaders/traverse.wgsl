// 64-tree DDA traversal with ancestor stack.
// Algorithm from dubiousconst282's sparse voxel tracing guide:
//   https://dubiousconst282.github.io/2024/10/03/voxel-ray-tracing/
//
// Key ideas:
//   - Ancestor stack caches parent node indices -- stepping to a neighbor doesn't restart from root.
//   - flip_mask mirrors coordinates so the ray is always positive, simplifying DDA stepping.
//   - Coarse occupancy groups the 64-bit mask into 8 2x2x2 regions for fast empty-space skipping.
//   - When a node has no uploaded children (LOD cutoff), read lod_material and shade as solid cube.

#include "types.wgsl"

struct TraceResult {
	hit: bool,
	material: u32,
	normal: vec3<f32>,
	t: f32,
}

fn trace(ray_origin: vec3<f32>, ray_dir: vec3<f32>) -> TraceResult {
	// todo
	return TraceResult(false, 0u, vec3<f32>(0.0), 0.0);
}
