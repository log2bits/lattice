use crate::tree::chunk::Chunk;

/// Collect all unique Voxel values into the chunk's MaterialTable.
/// Bitpack all index arrays (leaf_materials, lod_material) to ceil(log2(table_size)) bits.
/// Bitpack node_children to ceil(log2(pool_size)) bits per depth.
pub fn finalize_chunk(chunk: &mut Chunk, pools: &mut Vec<crate::tree::pool::NodePool>) {
	todo!()
}
