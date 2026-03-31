// High bit of a u32 child entry. When set, the lower bits are a material LUT
// index. When clear, the lower bits index into the next Level's node pool.
//
// How many of the lower bits are used for the material index is recorded in
// the .lattice header as mat_index_bits (always a power of two: 1, 2, 4, 8,
// 16, or 32). The GPU shader reads this once as a uniform and masks with
// (1 << mat_index_bits) - 1 to extract the index.
//
// A LEAF_FLAG child at any level means every voxel in that entire subtree has
// the same material. A solid stone mountain collapses to one entry.
pub const LEAF_FLAG: u32 = 1 << 31;

pub fn is_leaf_ref(child: u32) -> bool {
  child & LEAF_FLAG != 0
}

// Material LUT index from a leaf child entry. Only valid when is_leaf_ref() is true.
pub fn leaf_index(child: u32) -> u32 {
  child & !LEAF_FLAG
}

// Produces a child entry that points to a material. Store this in the children
// array, not the raw LUT index.
pub fn make_leaf_ref(idx: u32) -> u32 {
  idx | LEAF_FLAG
}

pub fn child_count(occupancy: u64) -> u32 {
  occupancy.count_ones()
}