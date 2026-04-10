#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugMode {
	#[default]
	None,
	Normals,
	Depth,
	LodDepth,
	Heatmap,
	GridLines,
}

/// Uniforms for the debug overlay shader.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct DebugUniforms {
	pub mode: u32,
}
