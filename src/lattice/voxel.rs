// Universal 32-bit voxel format. Every voxel in every scene uses this layout.
//
// Bit layout:
//   bits 31-8   rgb          24-bit linear RGB (R=31-24, G=23-16, B=15-8)
//   bits  7-4   roughness    nibble, 0 = perfect mirror, 15 = fully diffuse
//   bit   3     emissive     emits light at its albedo color
//   bit   2     metallic     conductor, albedo tints specular
//   bit   1     transparent  refracts rather than reflects
//   bit   0     reserved
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Voxel(pub u32);

impl Voxel {
	pub fn new(
		rgb: [u8; 3],
		roughness: u8,
		emissive: bool,
		metallic: bool,
		transparent: bool,
	) -> Self {
		Voxel(
			((rgb[0] as u32) << 24)
				| ((rgb[1] as u32) << 16)
				| ((rgb[2] as u32) << 8)
				| (((roughness & 0xF) as u32) << 4)
				| ((emissive as u32) << 3)
				| ((metallic as u32) << 2)
				| ((transparent as u32) << 1),
		)
	}

	pub fn rgb(self) -> [u8; 3] {
		[
			(self.0 >> 24) as u8,
			(self.0 >> 16) as u8,
			(self.0 >> 8) as u8,
		]
	}
	pub fn roughness(self) -> u8 {
		((self.0 >> 4) & 0xF) as u8
	}
	pub fn emissive(self) -> bool {
		(self.0 >> 3) & 1 != 0
	}
	pub fn metallic(self) -> bool {
		(self.0 >> 2) & 1 != 0
	}
	pub fn transparent(self) -> bool {
		(self.0 >> 1) & 1 != 0
	}
}

impl From<u32> for Voxel {
	fn from(v: u32) -> Self {
		Voxel(v)
	}
}

impl From<Voxel> for u32 {
	fn from(v: Voxel) -> Self {
		v.0
	}
}

// 256-entry perceptually uniform color palette in OKLab space, precomputed
// once using sample elimination. Used at import time to quantize scene colors,
// bounding the global voxel LUT to a manageable size.
#[derive(Clone, Debug, Default)]
pub struct ColorPalette {
	pub entries: Vec<[u8; 3]>,
}

impl ColorPalette {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn len(&self) -> u32 {
		self.entries.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	// Returns the index of the nearest palette entry to the given linear RGB.
	pub fn nearest(&self, rgb: [u8; 3]) -> u8 {
		todo!()
	}
}
