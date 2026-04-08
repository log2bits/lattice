// High bit of a u32 child entry. When set, the lower bits are a LUT index
// into the chunk's palette (uniform subtree or LOD terminal node).
// When clear, the lower bits are a pointer into the next level's node pool.
pub const LEAF_FLAG: u32 = 1 << 31;

pub fn is_leaf(child: u32) -> bool {
	child & LEAF_FLAG != 0
}

pub fn leaf_value(child: u32) -> u32 {
	child & !LEAF_FLAG
}

pub fn make_leaf(value: u32) -> u32 {
	value | LEAF_FLAG
}

pub fn child_count(occupancy: u64) -> u32 {
	occupancy.count_ones()
}
