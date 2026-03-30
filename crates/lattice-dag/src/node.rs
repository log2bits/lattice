// A node in the 64-tree. Every non-leaf position in the tree is one of these.
//
// The 64-bit occupancy mask has one bit per child slot. A set bit means that
// slot has a child. Children are packed into a flat array on the Level -- this
// node only stores where they start. The count is always occupancy.count_ones().
// To get this node's children: level.children[children_start .. children_start + child_count()].
//
// Each entry in that slice is a u32. The high bit says whether the child is
// another Node (next level down) or a leaf voxel value in the LeafPool.
pub struct Node {
  pub occupancy: u64,
  pub children_start: u32,
}

impl Node {
  pub fn child_count(&self) -> u32 {
    self.occupancy.count_ones()
  }
}

// High bit of a u32 child entry. When set, the lower 31 bits index into the
// LeafPool. When clear, they index into the next Level's node pool.
//
// A LEAF_FLAG child at any level means every voxel in that entire subtree has
// the same data as that one leaf. This is how uniform regions are represented
// without sentinel values or special cases.
//
// For geometry (T = ()), the LeafPool has one entry and every solid voxel
// points to LEAF_FLAG | 0. For color (T = Material), each unique material is
// one entry and a LEAF_FLAG anywhere in the tree means that entire region
// shares one material.
pub const LEAF_FLAG: u32 = 1 << 31;

pub fn is_leaf_ref(child: u32) -> bool {
  child & LEAF_FLAG != 0
}

// Index into the LeafPool. Only valid when is_leaf_ref() is true.
pub fn leaf_index(child: u32) -> u32 {
  child & !LEAF_FLAG
}

// Produces a child entry that points to a leaf. Store this in the children
// array, not the raw pool index.
pub fn make_leaf_ref(idx: u32) -> u32 {
  idx | LEAF_FLAG
}
