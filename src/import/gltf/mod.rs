pub mod material;
pub mod mesh;
pub mod voxelizer;

use crate::import::{ImportConfig, VoxelSample};
use std::path::Path;

// Loads a glTF scene and produces a sorted VoxelSample stream.
pub fn import_gltf(path: &Path, config: &ImportConfig) -> Result<Vec<VoxelSample>, anyhow::Error> {
	todo!()
}

pub(crate) fn bytes_per_pixel(format: gltf::image::Format) -> usize {
	match format {
		gltf::image::Format::R8 => 1,
		gltf::image::Format::R8G8 => 2,
		gltf::image::Format::R8G8B8 => 3,
		gltf::image::Format::R8G8B8A8 => 4,
		gltf::image::Format::R16 => 2,
		gltf::image::Format::R16G16 => 4,
		gltf::image::Format::R16G16B16 => 6,
		gltf::image::Format::R16G16B16A16 => 8,
		gltf::image::Format::R32G32B32FLOAT => 12,
		gltf::image::Format::R32G32B32A32FLOAT => 16,
		_ => 3,
	}
}
