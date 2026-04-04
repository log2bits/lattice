#![allow(unused)]

// Per-face GI accumulation state and update logic.
// Each voxel face stores a running weighted average of accumulated indirect light:
//   L_new = (1 - alpha) * L_old + alpha * S
// where S is the new path traced sample and alpha is tuned per surface type.

pub struct GiAccumulator {
  // One [f32; 3] color value per face. Indexed by (node_idx * 6 + face_idx).
  pub face_radiance: Vec<[f32; 3]>,
}

impl GiAccumulator {
  pub fn new(face_count: usize) -> Self {
    todo!()
  }

  // Blends a new sample into the running average for the given face.
  pub fn accumulate(&mut self, face_idx: usize, sample: [f32; 3], alpha: f32) {
    todo!()
  }
}
