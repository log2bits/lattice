// One leaf voxel, packed into a single u32.
//
// Bit layout:
//   31-15  normal      17-bit John White oct encoding (see below)
//   14     transparent refracts rather than reflects
//   13     metallic    conductor -- albedo tints specular
//   12     emissive    emits light at its albedo color
//   11-8   roughness   0 = perfect mirror, 15 = fully diffuse
//    7-0   palette     index into the scene's ColorPalette (256 entries)
//
// Normal encoding (John White signed octahedral):
//   Project normal onto L1 unit octahedron. Apply 45-degree rotation to
//   redistribute error uniformly. Store X and Y as u8 each, plus a sign
//   bit for Z. Decoding requires one normalize. Error is ~0.3 degrees
//   average, uniform across the sphere.
//
//   encode:
//     n = normalize(n) / (|nx| + |ny| + |nz|)
//     y = ny*0.5 + 0.5
//     x = nx*0.5 + y
//     y = nx*-0.5 + y
//     oct_x = round(x * 255)
//     oct_y = round(y * 255)
//     sgn_z = nz >= 0
//
//   decode (WGSL):
//     x = oct_x / 255.0
//     y = oct_y / 255.0
//     nx = x - y
//     ny = x + y - 1.0
//     nz = select(-1.0, 1.0, sgn_z) * (1.0 - |nx| - |ny|)
//     n = normalize(vec3(nx, ny, nz))
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Voxel(pub u32);

impl Voxel {
  pub fn new(
    oct_x: u8, oct_y: u8, sgn_z: bool,
    transparent: bool,
    metallic: bool,
    emissive: bool,
    roughness: u8,
    palette: u8,
  ) -> Self {
    let normal = ((oct_x as u32) << 9) | ((oct_y as u32) << 1) | (sgn_z as u32);
    Voxel(
        (normal               << 15)
      | ((transparent as u32) << 14)
      | ((metallic    as u32) << 13)
      | ((emissive    as u32) << 12)
      | (((roughness & 0xF)   as u32) << 8)
      |   (palette            as u32)
    )
  }

  pub fn oct_x(self)      -> u8   { ((self.0 >> 24) & 0xFF) as u8 }
  pub fn oct_y(self)      -> u8   { ((self.0 >> 16) & 0xFF) as u8 }
  pub fn sgn_z(self)      -> bool { (self.0 >> 15) & 1 != 0 }
  pub fn transparent(self)-> bool { (self.0 >> 14) & 1 != 0 }
  pub fn metallic(self)   -> bool { (self.0 >> 13) & 1 != 0 }
  pub fn emissive(self)   -> bool { (self.0 >> 12) & 1 != 0 }
  pub fn roughness(self)  -> u8   { ((self.0 >> 8) & 0xF) as u8 }
  pub fn palette(self)    -> u8   { (self.0 & 0xFF) as u8 }
}

// The scene's color palette. 256 linear RGB entries. Fits in ~768 bytes,
// permanently in L1 cache. Built at voxelization time using median-cut
// quantization in OKLab space (perceptually uniform, minimizes visible banding).
#[derive(Clone, Debug)]
pub struct ColorPalette {
  pub entries: Vec<[u8; 3]>,
}

impl ColorPalette {
  pub fn new() -> Self {
    Self { entries: Vec::new() }
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }
}