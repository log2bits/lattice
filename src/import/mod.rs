pub mod cull;
pub mod gltf;
pub mod palette;
pub mod partition;
pub mod pbr;
pub mod voxelize;

use crate::voxel::Voxel;
use rayon::prelude::*;
use std::path::PathBuf;

pub struct ImportConfig {
  pub voxel_size: f32,
  pub depth: u8,
  /// Optional path to a palette PNG. If None, full 24-bit RGB is used.
  pub palette_path: Option<PathBuf>,
}

/// One voxelized sample: morton-ordered position within a chunk plus voxel value.
pub struct VoxelSample {
  pub morton: u64,
  pub voxel: Voxel,
}

/// Info returned by `import` alongside the chunk callback.
pub struct ImportInfo {
  pub world_min: [f32; 3],
  pub world_max: [f32; 3],
  pub grid_dims: [u32; 3],
  pub chunk_voxels: u32,
}

/// Run the full import pipeline: glTF -> per-chunk VoxelSample streams.
/// Calls `on_chunk(flat_chunk_index, samples)` for each non-empty chunk.
/// Returns scene metadata.
pub fn import(
  path: &std::path::Path,
  config: &ImportConfig,
  mut on_chunk: impl FnMut(u64, Vec<VoxelSample>),
) -> anyhow::Result<ImportInfo> {
  let scene = gltf::load(path)?;

  let world_min = scene.bounds_min;
  let world_max = scene.bounds_max;

  let chunk_voxels = 4u32.pow(config.depth as u32);
  let chunk_size_m = chunk_voxels as f32 * config.voxel_size;

  let grid_dims = std::array::from_fn(|i| {
    ((world_max[i] - world_min[i]) / chunk_size_m).ceil() as u32 + 1
  });

  let pal = config.palette_path.as_ref()
    .map(|p| palette::Palette::load_palette(p));

  let partition_map = partition::partition(&scene, world_min, config.voxel_size, grid_dims, chunk_voxels);

  // Voxelize all chunks in parallel, then hand results to callback serially
  let results: Vec<(u64, Vec<VoxelSample>)> = partition_map.bins
    .into_par_iter()
    .enumerate()
    .filter(|(_, bin)| !bin.is_empty())
    .map(|(flat_idx, tri_indices)| {
      let cx = (flat_idx as u32) % grid_dims[0];
      let cy = ((flat_idx as u32) / grid_dims[0]) % grid_dims[1];
      let cz = (flat_idx as u32) / (grid_dims[0] * grid_dims[1]);
      let origin = [cx * chunk_voxels, cy * chunk_voxels, cz * chunk_voxels];

      let mut samples = voxelize::voxelize_chunk(
        &scene, &tri_indices, origin, world_min, config.voxel_size, chunk_voxels, pal.as_ref(),
      );

      cull::cull_interior(&mut samples);
      (flat_idx as u64, samples)
    })
    .collect();

  for (chunk_idx, samples) in results {
    if !samples.is_empty() {
      on_chunk(chunk_idx, samples);
    }
  }

  Ok(ImportInfo { world_min, world_max, grid_dims, chunk_voxels })
}
