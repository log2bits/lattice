/// Axis-aligned bounding box of all mesh geometry in a glTF scene.
pub fn scene_bounds(path: &std::path::Path) -> anyhow::Result<([f32; 3], [f32; 3])> {
	todo!()
}

pub struct GltfScene {
	pub meshes: Vec<GltfMesh>,
	pub materials: Vec<GltfMaterial>,
}

pub struct GltfMesh {
	/// Triangle list: each element is [v0, v1, v2] in world space.
	pub triangles: Vec<[[f32; 3]; 3]>,
	/// UV coordinates per triangle vertex.
	pub uvs: Vec<[[f32; 2]; 3]>,
	pub material_idx: usize,
}

pub struct GltfMaterial {
	pub base_color: [f32; 4],
	pub roughness: f32,
	pub metallic: f32,
	pub emissive: [f32; 3],
	/// Decoded sRGB pixels: [r, g, b, a] per pixel.
	pub base_color_texture: Option<(Vec<[u8; 4]>, u32, u32)>,
}

pub fn load(path: &std::path::Path) -> anyhow::Result<GltfScene> {
	todo!()
}
