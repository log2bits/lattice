pub mod sort;
pub mod dag;
pub mod lut;
pub mod materials;
pub mod serialize;

use std::path::Path;
use crate::lattice::{Lattice, SectionConfig};
use crate::import::VoxelSample;

pub struct PackConfig {
  pub sections:  Vec<SectionConfig>,
  pub world_min: [i64; 3],
  pub world_max: [i64; 3],
}

// Builds a Lattice from a VoxelSample stream and writes it to a .lattice file.
pub fn pack(config: PackConfig, samples: Vec<VoxelSample>, out: &Path) -> Result<(), anyhow::Error> {
  todo!()
}
