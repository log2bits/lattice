use crate::tree::Coverage;

/// Sphere defined by center (voxel-grid integer coords) and radius.
/// Uses voxel-center convention: a voxel at `[x,y,z]` is inside if
/// `|p - center|^2 <= radius^2`.
pub fn sphere(center: [u32; 3], radius: u32) -> impl Fn([u32; 3], u32) -> Coverage {
	move |base, side| {
		let r2 = (radius as i64) * (radius as i64);
		let hi = side - 1;
		// Closest voxel-center in the AABB to center.
		let min_dist2: i64 = (0..3)
			.map(|i| {
				let c = center[i] as i64;
				let lo = base[i] as i64;
				let h = lo + hi as i64;
				let d = if c < lo {
					lo - c
				} else if c > h {
					c - h
				} else {
					0
				};
				d * d
			})
			.sum();
		if min_dist2 > r2 {
			return Coverage::None;
		}
		// Farthest voxel-center in the AABB from center.
		let max_dist2: i64 = (0..3)
			.map(|i| {
				let c = center[i] as i64;
				let lo = base[i] as i64;
				let h = lo + hi as i64;
				let d = (c - lo).abs().max((c - h).abs());
				d * d
			})
			.sum();
		if max_dist2 <= r2 {
			Coverage::Full
		} else {
			Coverage::Partial
		}
	}
}

/// Axis-aligned box filling voxels from `min` to `max` inclusive.
pub fn cube(min: [u32; 3], max: [u32; 3]) -> impl Fn([u32; 3], u32) -> Coverage {
	move |base, side| {
		let hi = base.map(|b| b + side - 1);
		if (0..3).any(|i| hi[i] < min[i] || base[i] > max[i]) {
			return Coverage::None;
		}
		if (0..3).all(|i| base[i] >= min[i] && hi[i] <= max[i]) {
			return Coverage::Full;
		}
		Coverage::Partial
	}
}
