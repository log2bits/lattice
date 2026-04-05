#![allow(unused)]
use std::io::Read;

// Parsed representation of the .lattice file header.
pub struct LatticeHeader {
	pub version: u16,
	pub world_min: [i64; 3],
	pub world_max: [i64; 3],
	pub voxel_bits: u8,
	pub sections: Vec<SectionDesc>,
	pub levels: Vec<LevelDesc>,
	pub chunks: Vec<ChunkEntry>,
}

pub struct SectionDesc {
	pub layer_type: u8, // 0=Grid, 1=GeometryDag, 2=MaterialDag
	pub lut_enabled: u8,
	pub num_levels: u8,
}

pub struct LevelDesc {
	pub child_bits: u8,
}

pub struct ChunkEntry {
	pub tag: u32,
	pub offset: u64,
	pub size: u64,
}

// Parses the header from the start of a .lattice file.
pub fn parse_header(reader: &mut impl Read) -> Result<LatticeHeader, anyhow::Error> {
	todo!()
}
