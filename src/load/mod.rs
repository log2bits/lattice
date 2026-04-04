#![allow(unused)]
pub mod header;
pub mod stream;
pub mod upload;

use std::path::Path;
use crate::lattice::Lattice;
use upload::GpuLattice;

// Loads a .lattice file from disk and uploads it to the GPU.
pub fn load(path: &Path, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<GpuLattice, anyhow::Error> {
  todo!()
}
