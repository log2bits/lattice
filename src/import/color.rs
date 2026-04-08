use std::{collections::{HashMap, HashSet}, path::Path};

#[derive(Clone, Debug, Default)]
pub struct Palette {
	pub entries: Vec<[u8; 3]>,
	pub cache: HashMap<[u8; 3], [u8; 3]>,
}

impl Palette {
	pub fn new(entries: Vec<[u8; 3]>) -> Self {
		Self {
			entries,
			cache: HashMap::new()
		}
	}

	pub fn len(&self) -> u32 {
		self.entries.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	pub fn nearest(&mut self, rgb: [u8; 3]) -> [u8; 3] {
		if let Some(&nearest) = self.cache.get(&rgb) {
			return nearest;
		}
		let nearest = self.entries
			.iter()
			.copied()
			.min_by_key(|&entry| oklab_distance_sq(entry, rgb) as u32)
			.unwrap();
		self.cache.insert(rgb, nearest);
		nearest
	}

	pub fn load_palette(path: &Path) -> Palette {
    let img = image::open(path).expect("Failed to open palette image").to_rgb8();
    let mut seen = HashSet::new();
    let entries: Vec<[u8; 3]> = img
			.pixels()
			.map(|p| p.0)
			.filter(|rgb| seen.insert(*rgb))
			.collect();
    Palette::new(entries)
	}
}

fn srgb_to_linear(v: u8) -> f32 {
	let v = v as f32 / 255.0;
	if v <= 0.04045 {
		v / 12.92
	} else {
		((v + 0.055) / 1.055).powf(2.4)
	}
}

fn linear_to_srgb(v: f32) -> u8 {
	let v = if v <= 0.0031308 {
		12.92 * v
	} else {
		1.055 * v.powf(1.0 / 2.4) - 0.055
	};
	(v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

pub fn oklab_distance_sq(a: [u8; 3], b: [u8; 3]) -> f32 {
	#[inline]
	fn to_lms_cbrt(rgb: [u8; 3]) -> [f32; 3] {
		let r = srgb_to_linear(rgb[0]);
		let g = srgb_to_linear(rgb[1]);
		let b = srgb_to_linear(rgb[2]);
		[
			0.0514459929f32.mul_add(b, 0.4122214708f32.mul_add(r, 0.5363325363 * g)).cbrt(),
			0.1073969566f32.mul_add(b, 0.2119034982f32.mul_add(r, 0.6806995451 * g)).cbrt(),
			0.6299787005f32.mul_add(b, 0.0883024619f32.mul_add(r, 0.2817188376 * g)).cbrt(),
		]
	}
	let a = to_lms_cbrt(a);
	let b = to_lms_cbrt(b);
	let dl = a[0] - b[0];
	let dm = a[1] - b[1];
	let ds = a[2] - b[2];
	
	  0.6745098645 * dl * dl
	+ 6.4981939342 * dm * dm
	+ 1.0609939498 * ds * ds
	- 4.0572845340 * dl * dm
	+ 0.0521576466 * dl * ds
	- 4.4684798498 * dm * ds
}

pub fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
	let r = srgb_to_linear(rgb[0]);
	let g = srgb_to_linear(rgb[1]);
	let b = srgb_to_linear(rgb[2]);
	let l = 0.0514459929f32.mul_add(b, 0.4122214708f32.mul_add(r, 0.5363325363 * g));
	let m = 0.1073969566f32.mul_add(b, 0.2119034982f32.mul_add(r, 0.6806995451 * g));
	let s = 0.6299787005f32.mul_add(b, 0.0883024619f32.mul_add(r, 0.2817188376 * g));
	let l_ = l.cbrt();
	let m_ = m.cbrt();
	let s_ = s.cbrt();
	[
		(-0.0040720468f32).mul_add(s_, 0.2104542553f32.mul_add(l_,  0.7936177850 * m_)),
		( 0.4505937099f32).mul_add(s_, 1.9779984951f32.mul_add(l_, -2.4285922050 * m_)),
		(-0.8086757660f32).mul_add(s_, 0.0259040371f32.mul_add(l_,  0.7827717662 * m_)),
	]
}

pub fn oklab_to_rgb(oklab: [f32; 3]) -> [u8; 3] {
	let l_ = ( 0.2158037573f32).mul_add(oklab[2], ( 0.3963377774f32).mul_add(oklab[1], oklab[0]));
	let m_ = (-0.0638541728f32).mul_add(oklab[2], (-0.1055613458f32).mul_add(oklab[1], oklab[0]));
	let s_ = (-1.2914855480f32).mul_add(oklab[2], (-0.0894841775f32).mul_add(oklab[1], oklab[0]));
	let l = l_ * l_ * l_;
	let m = m_ * m_ * m_;
	let s = s_ * s_ * s_;
	[
		linear_to_srgb(( 0.2309699292f32).mul_add(s, ( 4.0767416621f32).mul_add(l, -3.3077115913 * m))),
		linear_to_srgb((-0.3413193965f32).mul_add(s, (-1.2684380046f32).mul_add(l,  2.6097574011 * m))),
		linear_to_srgb(( 1.7076147010f32).mul_add(s, (-0.0041960863f32).mul_add(l, -0.7034186147 * m))),
	]
}