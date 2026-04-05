#![allow(unused)]
use crate::import::VoxelSample;
use crate::lattice::{GeometryDagLevel, Lattice, MaterialDagLevel, SectionConfig};

// Builds the full DAG structure bottom-up from a Morton-sorted VoxelSample stream.
// Dispatches to the correct insert logic based on each section's LayerType.
pub fn build_dag(configs: &[SectionConfig], samples: &[VoxelSample]) -> Lattice {
	todo!()
}

// Inserts a batch of children into a GeometryDag level, deduplicating on
// occupancy only (geometry-only hash).
pub fn insert_geometry_dag(
	level: &mut GeometryDagLevel,
	occupancy: u64,
	voxel_count: u32,
	children: &[u32],
) -> u32 {
	todo!()
}

// Inserts a batch of children into a MaterialDag level, deduplicating on
// both occupancy and children (geometry + material hash).
pub fn insert_material_dag(level: &mut MaterialDagLevel, occupancy: u64, children: &[u32]) -> u32 {
	todo!()
}
