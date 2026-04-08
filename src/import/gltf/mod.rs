pub mod material;
pub mod mesh;
pub mod voxelizer;

use crate::import::color::Palette;
use crate::import::gltf::material::prepare_materials;
use crate::import::gltf::mesh::{Triangle, clip_to_chunk, extract_triangles};
use crate::import::gltf::voxelizer::voxelize_triangle;
use crate::import::{ImportConfig, VoxelSample};
use crate::pack::sort::morton_encode;
use std::collections::HashMap;
use std::path::Path;

// Loads a glTF scene and calls on_chunk for each voxelization chunk in Morton order.
// Each chunk's samples are sorted in Morton order before the callback fires.
// Chunks are emitted in Morton order, so the packer sees a globally ordered stream.
pub fn import_gltf(path: &Path, config: &ImportConfig, mut on_chunk: impl FnMut(Vec<VoxelSample>)) -> Result<(), anyhow::Error> {
	let mut palette = Palette::load_palette(&config.palette_path);
	let (document, buffers, images) = gltf::import(path)?;
	let materials = prepare_materials(&document, &images);

	// Phase 1: extract all triangles into a flat list, bin by chunk coord.
	let mut flat_triangles: Vec<Triangle> = Vec::new();
	let mut bins: HashMap<[i32; 3], Vec<usize>> = HashMap::new();

	for mesh in document.meshes() {
		for primitive in mesh.primitives() {
			let tris = extract_triangles(&primitive, &buffers);
			for tri in tris {
				for coord in triangle_chunk_overlap(&tri, config) {
					bins.entry(coord).or_default().push(flat_triangles.len());
				}
				flat_triangles.push(tri);
			}
		}
	}

	// Phase 2: process chunks in Morton order, emit one sorted run per chunk.
	let mut chunk_coords: Vec<[i32; 3]> = bins.keys().copied().collect();
	chunk_coords.sort_by_key(|&c| morton_encode([c[0] as i64, c[1] as i64, c[2] as i64]));

	for coord in chunk_coords {
		let indices = &bins[&coord];
		let vmin = chunk_voxel_min(coord, config.chunk_size);
		let vmax = [
			vmin[0] + config.chunk_size as i64,
			vmin[1] + config.chunk_size as i64,
			vmin[2] + config.chunk_size as i64,
		];
		let wmin = voxel_to_world(vmin, config.voxel_size);
		let wmax = voxel_to_world(vmax, config.voxel_size);

		let mut samples: Vec<VoxelSample> = Vec::new();
		for &tri_idx in indices {
			let tri = &flat_triangles[tri_idx];
			let mat = &materials[tri.material_idx];
			for clipped in clip_to_chunk(tri, wmin, wmax) {
				voxelize_triangle(&clipped, mat, &mut palette, config.voxel_size, vmin, vmax, &mut samples);
			}
		}

		// Morton sort, then deduplicate: one sample per voxel position.
		samples.sort_by_key(|s| morton_encode(s.position));
		samples.dedup_by(|a, b| a.position == b.position);

		on_chunk(samples);
	}

	Ok(())
}

// Returns the chunk-space coords of every chunk cell overlapping this triangle's AABB.
fn triangle_chunk_overlap(tri: &Triangle, config: &ImportConfig) -> Vec<[i32; 3]> {
	let mut aabb_min = tri.verts[0];
	let mut aabb_max = tri.verts[0];
	for &v in &tri.verts[1..] {
		for i in 0..3 {
			aabb_min[i] = aabb_min[i].min(v[i]);
			aabb_max[i] = aabb_max[i].max(v[i]);
		}
	}
	let lo = world_to_chunk(aabb_min, config);
	let hi = world_to_chunk(aabb_max, config);
	let mut coords = Vec::new();
	for z in lo[2]..=hi[2] {
		for y in lo[1]..=hi[1] {
			for x in lo[0]..=hi[0] {
				coords.push([x, y, z]);
			}
		}
	}
	coords
}

fn world_to_chunk(pos: [f32; 3], config: &ImportConfig) -> [i32; 3] {
	let cs = config.chunk_size as f64 * config.voxel_size;
	[
		(pos[0] as f64 / cs).floor() as i32,
		(pos[1] as f64 / cs).floor() as i32,
		(pos[2] as f64 / cs).floor() as i32,
	]
}

fn chunk_voxel_min(coord: [i32; 3], chunk_size: u32) -> [i64; 3] {
	[
		coord[0] as i64 * chunk_size as i64,
		coord[1] as i64 * chunk_size as i64,
		coord[2] as i64 * chunk_size as i64,
	]
}

fn voxel_to_world(voxel: [i64; 3], voxel_size: f64) -> [f32; 3] {
	[
		(voxel[0] as f64 * voxel_size) as f32,
		(voxel[1] as f64 * voxel_size) as f32,
		(voxel[2] as f64 * voxel_size) as f32,
	]
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
