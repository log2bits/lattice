use crate::material_table::MaterialTable;

/// One chunk: root node index into the depth-0 NodePool, plus per-chunk material table.
pub struct Chunk {
	pub root: u32,
	pub materials: MaterialTable,
}
