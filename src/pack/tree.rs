#![allow(unused)]
use crate::import::VoxelSample;
use crate::lattice::{Lattice, Level};

// Builds the SVO bottom-up from a Morton-sorted VoxelSample stream.
// rep_material is computed bottom-up from children. Uniform subtrees emit a
// LUT index into lut_children instead of a node pointer into ptr_children.
pub fn build_tree(depth: u8, samples: &[VoxelSample]) -> Lattice {
	todo!()
}

// Appends a node to a level. Returns its index.
pub fn insert_node(
	level: &mut Level,
	child_mask: u64,
	leaf_mask: u64,
	rep_material: u32,
	ptr_children: &[u32],
	lut_children: &[u32],
) -> u32 {
	todo!()
}
