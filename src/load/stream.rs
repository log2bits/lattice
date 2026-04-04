#![allow(unused)]
use std::io::Read;

// Decodes a PSVDAG DFS stream back into an explicit children array with node
// indices, ready for GPU upload.
pub fn decode_psvdag(stream: &[u8], node_count: usize) -> Vec<u32> {
  todo!()
}
