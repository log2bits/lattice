use std::collections::HashSet;
use std::path::Path;
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct Palette {
	pub entries: Vec<[u8; 3]>,
	lut: Box<[u8]>, // 16,777,216 entries, indexed by r<<16 | g<<8 | b
}

impl Palette {
	pub fn new(entries: Vec<[u8; 3]>) -> Self {
		let lut = Self::build_lut(&entries);
		Self { entries, lut }
	}

	fn build_lut(entries: &[[u8; 3]]) -> Box<[u8]> {
		if entries.is_empty() {
			return vec![0u8; 1 << 24].into_boxed_slice();
		}
		let palette_lab: Vec<[f32; 3]> = entries.iter().map(|&e| rgb_to_oklab(e)).collect();

		(0u32..16_777_216)
			.into_par_iter()
			.map(|i| {
				let sample = rgb_to_oklab([(i >> 16) as u8, (i >> 8) as u8, i as u8]);
				palette_lab.iter().enumerate()
					.min_by_key(|(_, e)| {
						let dl = sample[0] - e[0];
						let da = sample[1] - e[1];
						let db = sample[2] - e[2];
						(dl * dl + da * da + db * db).to_bits()
					})
					.map(|(i, _)| i as u8)
					.unwrap_or(0)
			})
			.collect::<Vec<u8>>()
			.into_boxed_slice()
	}

	pub fn len(&self) -> u32 {
		self.entries.len() as u32
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}

	pub fn nearest(&self, rgb: [u8; 3]) -> [u8; 3] {
		self.entries[self.nearest_idx(rgb) as usize]
	}

	pub fn nearest_idx(&self, rgb: [u8; 3]) -> u8 {
		self.lut[(rgb[0] as usize) << 16 | (rgb[1] as usize) << 8 | rgb[2] as usize]
	}

	pub fn load_palette(path: &Path) -> Self {
		let img = image::open(path).expect("failed to open palette image").to_rgb8();
		let mut seen = HashSet::new();
		let entries = img
			.pixels()
			.map(|p| p.0)
			.filter(|rgb| seen.insert(*rgb))
			.collect();
		Self::new(entries)
	}
}

#[inline(always)]
pub fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
	let dl = a[0] - b[0];
	let da = a[1] - b[1];
	let db = a[2] - b[2];
	dl * dl + da * da + db * db
}

#[inline(always)]
pub fn srgb_to_linear(v: u8) -> f32 {
	static TABLE: std::sync::LazyLock<[f32; 256]> = std::sync::LazyLock::new(|| {
		std::array::from_fn(|i| {
			let v = i as f32 / 255.0;
			if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
		})
	});
	TABLE[v as usize]
}

#[inline(always)]
pub fn linear_to_srgb(v: f32) -> u8 {
	let v = if v <= 0.0031308 {
		12.92 * v
	} else {
		1.055 * v.powf(1.0 / 2.4) - 0.055
	};
	(v * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

#[inline(always)]
pub fn to_lms_cbrt(rgb: [u8; 3]) -> [f32; 3] {
	let r = srgb_to_linear(rgb[0]);
	let g = srgb_to_linear(rgb[1]);
	let b = srgb_to_linear(rgb[2]);
	[
		0.0514459929f32.mul_add(b, 0.4122214708f32.mul_add(r, 0.5363325363 * g)).cbrt(),
		0.1073969566f32.mul_add(b, 0.2119034982f32.mul_add(r, 0.6806995451 * g)).cbrt(),
		0.6299787005f32.mul_add(b, 0.0883024619f32.mul_add(r, 0.2817188376 * g)).cbrt(),
	]
}

// squared oklab distance as a quadratic form in LMS^(1/3).
// coefficients are M^T M where M is the lms_cbrt -> oklab matrix.
#[inline(always)]
pub fn lms_dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
	let dl = a[0] - b[0];
	let dm = a[1] - b[1];
	let ds = a[2] - b[2];
	 3.9574400593 * dl * dl
	+ 7.1406209248 * dm * dm
	+ 0.8570077675 * ds * ds
	- 9.2329090758 * dl * dm
	+ 1.7389374669 * dl * ds
	- 3.4610971558 * dm * ds
}

#[inline(always)]
pub fn oklab_distance_sq(a: [u8; 3], b: [u8; 3]) -> f32 {
	lms_dist_sq(to_lms_cbrt(a), to_lms_cbrt(b))
}

#[inline(always)]
fn fast_cbrt(x: f32) -> f32 {
	// Newton-iteration approximation from the oklab crate -- avoids hardware cbrt
	const B: u32 = 709957561;
	const C: f32 = 5.4285717010e-1;
	const D: f32 = -7.0530611277e-1;
	const E: f32 = 1.4142856598e+0;
	const F: f32 = 1.6071428061e+0;
	const G: f32 = 3.5714286566e-1;
	let mut t = f32::from_bits((x.to_bits() / 3).wrapping_add(B));
	let s = C + (t * t) * (t / x);
	t *= G + F / (s + E + D / s);
	t
}

#[inline(always)]
pub fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
	let r = srgb_to_linear(rgb[0]);
	let g = srgb_to_linear(rgb[1]);
	let b = srgb_to_linear(rgb[2]);
	let l = 0.0514459929f32.mul_add(b, 0.4122214708f32.mul_add(r, 0.5363325363 * g));
	let m = 0.1073969566f32.mul_add(b, 0.2119034982f32.mul_add(r, 0.6806995451 * g));
	let s = 0.6299787005f32.mul_add(b, 0.0883024619f32.mul_add(r, 0.2817188376 * g));
	let l_ = fast_cbrt(l);
	let m_ = fast_cbrt(m);
	let s_ = fast_cbrt(s);
	[
		(-0.0040720468f32).mul_add(s_, 0.2104542553f32.mul_add(l_, 0.7936177850 * m_)),
		( 0.4505937099f32).mul_add(s_, 1.9779984951f32.mul_add(l_, -2.4285922050 * m_)),
		(-0.8086757660f32).mul_add(s_, 0.0259040371f32.mul_add(l_, 0.7827717662 * m_)),
	]
}

#[inline(always)]
pub fn oklab_to_rgb(oklab: [f32; 3]) -> [u8; 3] {
	let l_ = ( 0.2158037573f32).mul_add(oklab[2], ( 0.3963377774f32).mul_add(oklab[1], oklab[0]));
	let m_ = (-0.0638541728f32).mul_add(oklab[2], (-0.1055613458f32).mul_add(oklab[1], oklab[0]));
	let s_ = (-1.2914855480f32).mul_add(oklab[2], (-0.0894841775f32).mul_add(oklab[1], oklab[0]));
	let l = l_ * l_ * l_;
	let m = m_ * m_ * m_;
	let s = s_ * s_ * s_;
	[
		linear_to_srgb(( 0.2309699292f32).mul_add(s, ( 4.0767416621f32).mul_add(l, -3.3077115913 * m))),
		linear_to_srgb((-0.3413193965f32).mul_add(s, (-1.2684380046f32).mul_add(l, 2.6097574011 * m))),
		linear_to_srgb(( 1.7076147010f32).mul_add(s, (-0.0041960863f32).mul_add(l, -0.7034186147 * m))),
	]
}

#[inline(always)]
pub fn idx_to_rgb(idx: usize) -> [u8; 3] {
	[(idx >> 16) as u8, (idx >> 8) as u8, idx as u8]
}

#[inline(always)]
pub fn rgb_to_idx(rgb: [u8; 3]) -> usize {
	((rgb[0] as usize) << 16) | ((rgb[1] as usize) << 8) | rgb[2] as usize
}