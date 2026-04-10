// Fullscreen blit with basic tonemapping from the compute output texture to the swapchain.

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct VertexOut {
	@builtin(position) pos: vec4<f32>,
	@location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
	// Fullscreen triangle.
	let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
	return VertexOut(vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0), uv);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
	let color = textureSample(src, src_sampler, in.uv).rgb;
	// todo: tonemap
	return vec4<f32>(color, 1.0);
}
