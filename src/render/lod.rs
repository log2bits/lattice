use crate::render::camera::Camera;
use crate::tree::grid::Grid;

/// Compute target upload depth for a chunk based on camera distance and screen-space projected size.
pub fn target_depth(grid: &Grid, chunk_idx: u32, camera: &Camera, max_depth: u8) -> u8 {
	todo!()
}

/// Walk the grid and compute target depth for every chunk.
pub fn compute_target_depths(grid: &Grid, camera: &Camera, max_depth: u8) -> Vec<u8> {
	todo!()
}
