#![allow(unused)]
use crate::import::VoxelSample;

// Sorts a VoxelSample stream into Morton (Z-curve) order.
// The pack stage requires Morton order for bottom-up DAG construction.
pub fn sort_morton(samples: Vec<VoxelSample>) -> Vec<VoxelSample> {
  todo!()
}

// Encodes a 3D integer coordinate as a 64-bit Morton code.
pub fn morton_encode(pos: [i64; 3]) -> u64 {
  todo!()
}

// Decodes a 64-bit Morton code back to a 3D integer coordinate.
pub fn morton_decode(code: u64) -> [i64; 3] {
  todo!()
}
