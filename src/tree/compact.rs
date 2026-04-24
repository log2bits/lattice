use super::Tree;

impl<const DEPTH: usize> Tree<DEPTH> {
	// Rebuild all level arrays keeping only nodes reachable from root.
	// Removes orphans left behind by partial-rebuild edit walks.
	pub fn compact(&mut self) {
		todo!()
	}
}
