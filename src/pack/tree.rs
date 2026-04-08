#![allow(unused)]
use crate::import::VoxelSample;
use crate::lattice::{Lattice, Level};

// Builds the SVO bottom-up from a Morton-sorted VoxelSample stream.
// Material data is packed inline: rep_material is computed bottom-up from
// children, and uniform subtrees emit LEAF_FLAG | lut_index child entries.
pub fn build_tree(depth: u8, samples: &[VoxelSample]) -> Lattice {
	todo!()
}

// Appends a node to a level. Returns its index.
pub fn insert_node(level: &mut Level, child_mask: u64, rep_material: u32, children: &[u32]) -> u32 {
	todo!()
}
