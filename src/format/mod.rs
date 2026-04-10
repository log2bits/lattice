pub mod read;
pub mod write;
#[cfg(feature = "import")]
pub mod vox;

pub const MAGIC: &[u8; 8] = b"LATTICE\0";
pub const VERSION: u32 = 1;

/// .lattice file header.
pub struct LatticeHeader {
	pub version: u32,
	pub depth: u8,
	pub grid_dims: [u32; 3],
	pub chunk_count: u32,
	pub voxel_size_m: f32,
}
