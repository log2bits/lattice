#![allow(unused)]
use crate::lattice::{GeometryDagLevel, Lattice};

// Repacks a level's children array to the minimum bit width needed to address
// the pool. Called after DAG construction when the node pool size is known.
pub fn repack_pool_children(level: &mut GeometryDagLevel) {
	todo!()
}
