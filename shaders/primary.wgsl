// Primary ray compute shader. One thread per pixel.
// Sets up camera rays and calls trace() from traverse.wgsl.

#include "types.wgsl"
#include "traverse.wgsl"

struct CameraUniforms {
	origin:  vec4<f32>,
	forward: vec4<f32>,
	right:   vec4<f32>,
	up:      vec4<f32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniforms;
@group(0) @binding(1) var output: texture_storage_2d<rgba8unorm, write>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
	let dims = textureDimensions(output);
	if id.x >= dims.x || id.y >= dims.y {
		return;
	}

	// todo: cast ray, shade, write output
}
