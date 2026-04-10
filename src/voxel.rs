/// 32-bit voxel value.
///
/// bits 31-8: rgb (24-bit linear)
/// bits  7-4: roughness nibble (0 = mirror, 15 = fully diffuse)
/// bit      3: emissive
/// bit      2: metallic
/// bit      1: transparent
/// bit      0: reserved
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Voxel(u32);

impl Voxel {
	pub fn from_rgb_flags(rgb: [u8; 3], roughness: u8, emissive: bool, metallic: bool, transparent: bool) -> Self {
		todo!()
	}

	pub fn rgb(self) -> [u8; 3] {
		todo!()
	}

	pub fn roughness(self) -> u8 {
		todo!()
	}

	pub fn emissive(self) -> bool {
		todo!()
	}

	pub fn metallic(self) -> bool {
		todo!()
	}

	pub fn transparent(self) -> bool {
		todo!()
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
