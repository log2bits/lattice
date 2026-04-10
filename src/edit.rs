use crate::tree::Lattice;
use crate::voxel::Voxel;

/// Place a voxel at world position. Extends the chunk MaterialTable if it's a new unique value.
/// Walks down to the target leaf, modifies it, walks back up updating lod_material.
pub fn place_voxel(lattice: &mut Lattice, pos: [u32; 3], voxel: Voxel) {
	todo!()
}

/// Remove the voxel at world position. Walks down, clears the leaf, walks back up.
pub fn remove_voxel(lattice: &mut Lattice, pos: [u32; 3]) {
	todo!()
}
