#![allow(unused)]
use crate::import::VoxelSample;
use crate::lattice::{GeometryDagLevel, Lattice};

// Builds the geometry DAG bottom-up from a Morton-sorted VoxelSample stream.
// Deduplicates on occupancy only across all levels. Material data is collected
// into each root's MaterialsArray in Dolonius DFS order.
pub fn build_dag(dag_depth: u8, samples: &[VoxelSample]) -> Lattice {
	todo!()
}

// Inserts a node into a geometry DAG level, deduplicating on occupancy only.
// Returns the node index.
pub fn insert_node(
	level: &mut GeometryDagLevel,
	occupancy: u64,
	voxel_count: u32,
	children: &[u32],
) -> u32 {
	todo!()
}
