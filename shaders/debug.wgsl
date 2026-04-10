// Debug overlay shaders: normals, depth, LOD depth per chunk, traversal heatmap, grid lines.

#include "types.wgsl"

struct DebugUniforms {
	mode: u32, // 0=none 1=normals 2=depth 3=lod_depth 4=heatmap 5=grid_lines
}

@group(0) @binding(0) var<uniform> debug: DebugUniforms;

// todo: overlay compute or fragment shader
