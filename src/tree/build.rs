use super::Tree;

impl Tree {
	// Build bottom-up from tree-order (Morton-sorted) (key, value) pairs.
	// Leaf nodes first; parents collapse 64 children into one node, dropping
	// uniform groups to a single terminal entry.
	pub fn build_from_voxels(depth: u8, voxels: &[(u64, u32)]) -> Self { todo!() }
}
