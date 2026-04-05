#![allow(unused)]
use crate::lattice::{Lattice, Section};

// Builds the global voxel LUT by deduplicating all Voxel values across all sections.
// Sets voxel_bits on the Lattice and updates all section leaf entries and materials
// array entries to be global voxel LUT indices.
pub fn build_global_voxel_lut(lattice: &mut Lattice) {
	todo!()
}

// Builds per-section-root LUTs for sections with lut_enabled.
// For each section root, collects the unique global voxel LUT indices within
// its subtree, stores them in SectionRoot.lut_entries, and repacks the leaf
// entries in the bottom level's children array to use local bit widths.
pub fn build_section_root_luts(section: &mut Section) {
	todo!()
}

// Repacks a level's children array to the minimum bit width for the pool size.
// Called after DAG construction when the node pool size is known.
pub fn repack_pool_children(section: &mut Section) {
	todo!()
}
