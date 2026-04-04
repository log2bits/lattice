#![allow(unused)]
use std::path::Path;
use crate::lattice::Lattice;

// Writes a fully-built Lattice to a .lattice file using PSVDAG encoding for
// the node children arrays.
pub fn write_lattice(lattice: &Lattice, path: &Path) -> Result<(), anyhow::Error> {
  todo!()
}

// Encodes the node children array of a level using PSVDAG: nodes are written
// depth-first, with back-references for repeated nodes.
pub fn encode_psvdag(children: &[u32], node_count: usize) -> Vec<u8> {
  todo!()
}
