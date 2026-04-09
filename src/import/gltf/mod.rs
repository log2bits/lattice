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

// Computes the scene AABB in voxel-space by walking the scene graph and applying
// node transforms to each primitive's bounding box corners.
// Returns (world_min, world_max) as integer voxel coordinates.
pub fn gltf_scene_bounds(path: &Path, voxel_size: f64) -> Result<([i64; 3], [i64; 3]), anyhow::Error> {
	let (document, _buffers, _images) = gltf::import(path)?;

	let mut scene_min = [f32::MAX; 3];
	let mut scene_max = [f32::MIN; 3];

	// Walk every scene root, accumulating the transform down the node tree.
	let identity = [
		[1.0f32, 0.0, 0.0, 0.0],
		[0.0, 1.0, 0.0, 0.0],
		[0.0, 0.0, 1.0, 0.0],
		[0.0, 0.0, 0.0, 1.0],
	];
	for scene in document.scenes() {
		for node in scene.nodes() {
			visit_node(&node, &identity, &mut scene_min, &mut scene_max);
		}
	}

	if scene_min[0] == f32::MAX {
		return Ok(([0, 0, 0], [1, 1, 1]));
	}

	let world_min = [
		(scene_min[0] as f64 / voxel_size).floor() as i64,
		(scene_min[1] as f64 / voxel_size).floor() as i64,
		(scene_min[2] as f64 / voxel_size).floor() as i64,
	];
	let world_max = [
		(scene_max[0] as f64 / voxel_size).ceil() as i64,
		(scene_max[1] as f64 / voxel_size).ceil() as i64,
		(scene_max[2] as f64 / voxel_size).ceil() as i64,
	];

	Ok((world_min, world_max))
}

fn visit_node(
	node: &gltf::Node,
	parent_transform: &[[f32; 4]; 4],
	scene_min: &mut [f32; 3],
	scene_max: &mut [f32; 3],
) {
	let local = node.transform().matrix();
	let transform = mat4_mul(parent_transform, &local);

	if let Some(mesh) = node.mesh() {
		for primitive in mesh.primitives() {
			let bb = primitive.bounding_box();
			// Transform all 8 corners of the local AABB into world space.
			for &sx in &[bb.min[0], bb.max[0]] {
				for &sy in &[bb.min[1], bb.max[1]] {
					for &sz in &[bb.min[2], bb.max[2]] {
						let w = mat4_transform_point(&transform, [sx, sy, sz]);
						for i in 0..3 {
							scene_min[i] = scene_min[i].min(w[i]);
							scene_max[i] = scene_max[i].max(w[i]);
						}
					}
				}
			}
		}
	}

	for child in node.children() {
		visit_node(&child, &transform, scene_min, scene_max);
	}
}

// gltf matrices are column-major: m[col][row]
fn mat4_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
	let mut out = [[0.0f32; 4]; 4];
	for col in 0..4 {
		for row in 0..4 {
			for k in 0..4 {
				out[col][row] += a[k][row] * b[col][k];
			}
		}
	}
	out
}

fn mat4_transform_point(m: &[[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
	// m[col][row], p is extended to [x, y, z, 1]
	let w = m[0][3] * p[0] + m[1][3] * p[1] + m[2][3] * p[2] + m[3][3];
	[
		(m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0]) / w,
		(m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1]) / w,
		(m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2]) / w,
	]
}

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
