use crate::voxel::Voxel;

pub struct PbrSample {
  pub rgb: [u8; 3],
  pub roughness: f32,
  pub metallic: f32,
  pub emissive: bool,
}

/// Convert PBR material properties to the Voxel bit layout.
pub fn pbr_to_voxel(sample: &PbrSample) -> Voxel {
  let roughness_nibble = ((sample.roughness * 15.0).round() as u8).min(15);
  let metallic = sample.metallic > 0.5;
  Voxel::from_rgb_flags(sample.rgb, roughness_nibble, sample.emissive, metallic, false)
}
