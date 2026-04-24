use super::Tree;

impl Tree {
	// Walk top-down applying a sorted list of (morton_key, value) edits.
	// Nodes with no edits in their range are copied unchanged. Cost is O(depth * edits).
	pub fn apply_sorted_edits(&mut self, edits: &[(u64, u32)]) {
		todo!()
	}
}
