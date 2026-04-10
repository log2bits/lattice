/// Set on a grid entry to mark it as a proxy (only lod_material metadata loaded, no full tree).
pub const PROXY_FLAG: u32 = 1 << 31;

/// Set in node_children or leaf_materials to indicate a uniform solid subtree.
pub const SOLID_FLAG: u32 = 1 << 31;

/// Convert a 3D slot (x, y, z each 0..4) to a flat index 0..64.
#[inline]
pub fn slot_index(x: u8, y: u8, z: u8) -> u8 {
	todo!()
}

/// Decode a flat slot index to (x, y, z) within a 4x4x4 block.
#[inline]
pub fn slot_coords(slot: u8) -> (u8, u8, u8) {
	todo!()
}

/// Compress 64 occupancy bits into 8 coarse 2x2x2 region bits for fast skipping.
#[inline]
pub fn coarse_occupancy(occupancy: u64) -> u8 {
	todo!()
}
