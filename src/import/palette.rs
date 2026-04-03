use palette::{FromColor, IntoColor, Oklab, Oklch, Srgb};
use rand::prelude::*;
use rayon::prelude::*;

const NUM_COLORS: usize = 256 * 256 * 256;

fn idx_to_rgb(idx: usize) -> [u8; 3] {
  [(idx >> 16) as u8, (idx >> 8) as u8, idx as u8]
}

fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
  let srgb = Srgb::new(rgb[0], rgb[1], rgb[2]).into_format::<f32>();
  let lab: Oklab = srgb.into_color();
  [lab.l, lab.a, lab.b]
}

// Convert an OKLAB centroid back to sRGB. If it's outside the gamut (which
// happens when the mean of a cluster drifts across the boundary), binary-search
// in OKLCH to reduce chroma until it lands in-gamut. No clamping ever happens.
fn oklab_to_srgb(lab: [f32; 3]) -> [u8; 3] {
  let oklab = Oklab::new(lab[0], lab[1], lab[2]);
  let srgb: Srgb<f32> = Srgb::from_color(oklab);
  let r = (srgb.red   * 255.0).round() as i32;
  let g = (srgb.green * 255.0).round() as i32;
  let b = (srgb.blue  * 255.0).round() as i32;
  if r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255 {
    return [r as u8, g as u8, b as u8];
  }
  let lch: Oklch = oklab.into_color();
  let mut lo = 0.0f32;
  let mut hi = lch.chroma;
  for _ in 0..24 {
    let mid = (lo + hi) / 2.0;
    let s: Srgb<f32> = Srgb::from_color(Oklab::from_color(Oklch::new(lch.l, mid, lch.hue)));
    if s.red >= 0.0 && s.red <= 1.0 && s.green >= 0.0 && s.green <= 1.0 && s.blue >= 0.0 && s.blue <= 1.0 {
      lo = mid;
    } else {
      hi = mid;
    }
  }
  let s: Srgb<f32> = Srgb::from_color(Oklab::from_color(Oklch::new(lch.l, lo, lch.hue)));
  [
    (s.red   * 255.0).round().clamp(0.0, 255.0) as u8,
    (s.green * 255.0).round().clamp(0.0, 255.0) as u8,
    (s.blue  * 255.0).round().clamp(0.0, 255.0) as u8,
  ]
}

#[inline(always)]
fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
  let dl = a[0] - b[0];
  let da = a[1] - b[1];
  let db = a[2] - b[2];
  dl * dl + da * da + db * db
}

// K-means++ initialization. No LUT — OKLAB computed on the fly per candidate.
// Each step updates min-distances in parallel, then does a weighted sample.
fn kmeans_init(lut: &[[f32; 3]], k: usize, rng: &mut SmallRng) -> Vec<[f32; 3]> {
  let mut centroids: Vec<[f32; 3]> = Vec::with_capacity(k);
  centroids.push(lut[rng.gen_range(0..NUM_COLORS)]);

  let mut min_dists = vec![f32::MAX; NUM_COLORS];

  for step in 1..k {
    let last = *centroids.last().unwrap();
    min_dists.par_iter_mut().enumerate().for_each(|(i, d)| {
      *d = d.min(dist_sq(lut[i], last));
    });

    let total: f64 = min_dists.iter().map(|&d| d as f64).sum();
    let mut threshold = rng.r#gen::<f64>() * total;
    let mut chosen = NUM_COLORS - 1;
    for (i, &d) in min_dists.iter().enumerate() {
      threshold -= d as f64;
      if threshold <= 0.0 { chosen = i; break; }
    }
    centroids.push(lut[chosen]);

    if (step + 1) % 32 == 0 || step + 1 == k {
      eprintln!("  k-means++ {}/{k}", step + 1);
    }
  }
  centroids
}

// Combined assign + accumulate in one parallel fold, so we only scan the
// 16M LUT once per iteration instead of twice.
fn kmeans_lloyd(lut: &[[f32; 3]], mut centroids: Vec<[f32; 3]>, max_iters: usize, tolerance: f32) -> Vec<[f32; 3]> {
  let k = centroids.len();
  let mut best_move = f32::MAX;
  let mut no_improve = 0usize;

  for iter in 0..max_iters {
    let (sums, counts) = lut
      .par_iter()
      .fold(
        || (vec![[0f64; 3]; k], vec![0usize; k]),
        |(mut sums, mut counts), &lab| {
          let c = centroids
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| dist_sq(**a, lab).total_cmp(&dist_sq(**b, lab)))
            .map(|(ci, _)| ci)
            .unwrap();
          sums[c][0] += lab[0] as f64;
          sums[c][1] += lab[1] as f64;
          sums[c][2] += lab[2] as f64;
          counts[c] += 1;
          (sums, counts)
        },
      )
      .reduce(
        || (vec![[0f64; 3]; k], vec![0usize; k]),
        |(mut s1, mut c1), (s2, c2)| {
          for i in 0..k { s1[i][0] += s2[i][0]; s1[i][1] += s2[i][1]; s1[i][2] += s2[i][2]; c1[i] += c2[i]; }
          (s1, c1)
        },
      );

    let mut max_move = 0.0f32;
    centroids = (0..k)
      .map(|i| {
        if counts[i] == 0 { return centroids[i]; }
        let new = [
          (sums[i][0] / counts[i] as f64) as f32,
          (sums[i][1] / counts[i] as f64) as f32,
          (sums[i][2] / counts[i] as f64) as f32,
        ];
        max_move = max_move.max(dist_sq(centroids[i], new).sqrt());
        new
      })
      .collect();

    eprintln!("  iter {}: max centroid move = {max_move:.6}", iter + 1);

    if max_move < tolerance { eprintln!("  Converged."); break; }
    if max_move < best_move { best_move = max_move; no_improve = 0; }
    else {
      no_improve += 1;
      if no_improve >= 5 { eprintln!("  Stopped: cycling."); break; }
    }
  }

  centroids
}

// Hungarian algorithm (Kuhn-Munkres, O(n^3)) for optimal assignment of
// palette colors to SOM grid cells.
fn hungarian(weights: &[[f32; 3]], labs: &[[f32; 3]]) -> Vec<usize> {
  let n = labs.len();
  let cost = |i: usize, j: usize| dist_sq(weights[i - 1], labs[j - 1]);
  let mut u   = vec![0.0f32; n + 1];
  let mut v   = vec![0.0f32; n + 1];
  let mut p   = vec![0usize; n + 1];
  let mut way = vec![0usize; n + 1];

  for i in 1..=n {
    p[0] = i;
    let mut j0 = 0usize;
    let mut minval = vec![f32::MAX; n + 1];
    let mut used   = vec![false; n + 1];
    loop {
      used[j0] = true;
      let i0 = p[j0];
      let mut delta = f32::MAX;
      let mut j1 = 0usize;
      for j in 1..=n {
        if !used[j] {
          let cur = cost(i0, j) - u[i0] - v[j];
          if cur < minval[j] { minval[j] = cur; way[j] = j0; }
          if minval[j] < delta { delta = minval[j]; j1 = j; }
        }
      }
      for j in 0..=n {
        if used[j] { u[p[j]] += delta; v[j] -= delta; }
        else { minval[j] -= delta; }
      }
      j0 = j1;
      if p[j0] == 0 { break; }
    }
    loop {
      let j1 = way[j0];
      p[j0] = p[j1];
      j0 = j1;
      if j0 == 0 { break; }
    }
  }

  let mut result = vec![0usize; n];
  for j in 1..=n {
    if p[j] != 0 { result[p[j] - 1] = j - 1; }
  }
  result
}

// Toroidal SOM: trains floating-point OKLAB weight vectors on the palette,
// then uses Hungarian assignment for an exact, artifact-free final mapping.
fn som_layout(labs: &[[f32; 3]]) -> Vec<usize> {
  let n    = labs.len();
  let side = (n as f32).sqrt() as usize;
  let mut rng = SmallRng::seed_from_u64(42);

  let mut weights: Vec<[f32; 3]> = labs.to_vec();
  weights.shuffle(&mut rng);

  let num_iters   = 200_000usize;
  let sigma_init  = side as f32 / 2.0;
  let sigma_final = 0.5f32;
  let lr_init     = 0.5f32;
  let lr_final    = 0.01f32;

  eprintln!("Training toroidal SOM ({num_iters} iterations)...");
  for t in 0..num_iters {
    let progress   = t as f32 / num_iters as f32;
    let sigma      = sigma_init * (sigma_final / sigma_init).powf(progress);
    let lr         = lr_init   * (lr_final   / lr_init  ).powf(progress);
    let two_sig_sq = 2.0 * sigma * sigma;

    let input = labs[rng.gen_range(0..n)];
    let bmu   = (0..n)
      .min_by(|&a, &b| dist_sq(weights[a], input).total_cmp(&dist_sq(weights[b], input)))
      .unwrap();

    let bmu_row = (bmu / side) as f32;
    let bmu_col = (bmu % side) as f32;

    for j in 0..n {
      let dr = { let d = ((j / side) as f32 - bmu_row).abs(); d.min(side as f32 - d) };
      let dc = { let d = ((j % side) as f32 - bmu_col).abs(); d.min(side as f32 - d) };
      let influence = (-(dr * dr + dc * dc) / two_sig_sq).exp();
      if influence < 1e-6 { continue; }
      for k in 0..3 {
        weights[j][k] += lr * influence * (input[k] - weights[j][k]);
      }
    }

    if (t + 1) % 50_000 == 0 {
      eprintln!("  {}/{num_iters} (sigma={sigma:.2}, lr={lr:.3})", t + 1);
    }
  }

  eprintln!("Computing optimal assignment (Hungarian)...");
  hungarian(&weights, labs)
}


// Generates a Hald CLUT PNG (level 8, 512x512) mapping every input color to
// its nearest palette color in OKLAB space. Compatible with darktable, GIMP,
// and any other tool that supports Hald CLUTs. Also used by the palettize
// binary for O(1) per-pixel quantization.
//
// Level 8 means 64 steps per channel (8^2), stored as a 512x512 image (8^3 x 8^3).
// Pixel at (x, y) encodes input:
//   r_idx = x % 64,  g_idx = y % 64,  b_idx = (y / 64) * 8 + x / 64
fn generate_hald_clut(palette_labs: &[[f32; 3]], palette_rgb: &[[u8; 3]]) {
  const LEVEL: usize = 8;
  const STEPS: usize = LEVEL * LEVEL; // 64 steps per channel
  const IMG_SIZE: usize = LEVEL * LEVEL * LEVEL; // 512

  eprintln!("Generating Hald CLUT (level {LEVEL}, {IMG_SIZE}x{IMG_SIZE})...");

  let pixels: Vec<(u32, u32, [u8; 3])> = (0..STEPS)
    .into_par_iter()
    .flat_map_iter(|b_idx| {
      (0..STEPS).flat_map(move |g_idx| {
        (0..STEPS).map(move |r_idx| {
          let r = (r_idx * 255 / (STEPS - 1)) as u8;
          let g = (g_idx * 255 / (STEPS - 1)) as u8;
          let b = (b_idx * 255 / (STEPS - 1)) as u8;

          let lab = rgb_to_oklab([r, g, b]);
          let best = palette_labs
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| dist_sq(**a, lab).total_cmp(&dist_sq(**b, lab)))
            .map(|(i, _)| i)
            .unwrap();

          let x = ((b_idx % LEVEL) * STEPS + r_idx) as u32;
          let y = ((b_idx / LEVEL) * STEPS + g_idx) as u32;
          (x, y, palette_rgb[best])
        })
      })
    })
    .collect();

  let mut img = image::RgbImage::new(IMG_SIZE as u32, IMG_SIZE as u32);
  for (x, y, [r, g, b]) in pixels {
    img.put_pixel(x, y, image::Rgb([r, g, b]));
  }
  img.save("palette_lut.png").expect("failed to save palette_lut.png");
  eprintln!("Saved palette_lut.png.");
}

pub fn generate(size: usize) -> Vec<[u8; 3]> {
  let side = (size as f32).sqrt() as usize;
  assert_eq!(side * side, size, "size must be a perfect square");

  let mut rng = SmallRng::seed_from_u64(42);

  eprintln!("Building OKLAB LUT ({NUM_COLORS} colors)...");
  let lut: Vec<[f32; 3]> = (0..NUM_COLORS)
    .into_par_iter()
    .map(|i| rgb_to_oklab(idx_to_rgb(i)))
    .collect();
  eprintln!("LUT ready (~{}MB).", std::mem::size_of_val(lut.as_slice()) / 1_000_000);

  eprintln!("Initializing {size} centroids (k-means++)...");
  let initial = kmeans_init(&lut, size, &mut rng);

  eprintln!("Running Lloyd's algorithm...");
  let centroids = kmeans_lloyd(&lut, initial, 50, 5e-4);

  eprintln!("Snapping {size} centroids to sRGB...");
  let colors: Vec<[u8; 3]> = centroids.iter().map(|&c| oklab_to_srgb(c)).collect();

  let labs: Vec<[f32; 3]> = colors.iter().map(|&rgb| rgb_to_oklab(rgb)).collect();
  let grid = som_layout(&labs);

  let path = "palette.png";
  let mut img = image::RgbImage::new(side as u32, side as u32);
  for (pos, &idx) in grid.iter().enumerate() {
    let [r, g, b] = colors[idx];
    img.put_pixel((pos % side) as u32, (pos / side) as u32, image::Rgb([r, g, b]));
  }
  img.save(path).expect("failed to save palette.png");

  let saved = image::open(path).expect("failed to re-open palette.png").into_rgb8();
  let mut set: std::collections::HashSet<[u8; 3]> = colors.iter().cloned().collect();
  let mut mismatches = 0;
  for pixel in saved.pixels() {
    let c = [pixel[0], pixel[1], pixel[2]];
    if !set.remove(&c) {
      eprintln!("  MISMATCH: #{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
      mismatches += 1;
    }
  }
  if mismatches == 0 && set.is_empty() {
    eprintln!("Verification passed: all {size} colors present and correct.");
  } else {
    if mismatches > 0 { eprintln!("Verification FAILED: {mismatches} pixel(s) not in original palette."); }
    if !set.is_empty() { eprintln!("Verification FAILED: {} color(s) missing from image.", set.len()); }
  }

  // Hald CLUT for O(1) per-pixel quantization of any image.
  let palette_labs: Vec<[f32; 3]> = colors.iter().map(|&rgb| rgb_to_oklab(rgb)).collect();
  generate_hald_clut(&palette_labs, &colors);

  colors
}