pub struct PointOfInterest {
	pub world_pos: [i64; 3],
	// Deepest tree level to resolve for this point. Camera = WORLD_DEPTH (full LOD-0).
	pub max_depth: u8,
}
