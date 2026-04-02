use oklab::{srgb_to_oklab, Rgb};
use rand::prelude::*;
use rayon::prelude::*;

const NUM_COLORS: usize = 256 * 256 * 256;

const SEEDS: &[[u8; 3]] = &[
  [0,   0,   0  ],
  [255, 0,   0  ],
  [0,   255, 0  ],
  [0,   0,   255],
  [255, 255, 0  ],
  [255, 0,   255],
  [0,   255, 255],
  [255, 255, 255],
];

fn idx_to_rgb(idx: usize) -> [u8; 3] {
  [(idx >> 16) as u8, (idx >> 8) as u8, idx as u8]
}

fn rgb_to_idx(rgb: [u8; 3]) -> usize {
  (rgb[0] as usize) << 16 | (rgb[1] as usize) << 8 | rgb[2] as usize
}

fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
  let lab = srgb_to_oklab(Rgb { r: rgb[0], g: rgb[1], b: rgb[2] });
  [lab.l, lab.a, lab.b]
}

#[inline(always)]
fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
  let dl = a[0] - b[0];
  let da = a[1] - b[1];
  let db = a[2] - b[2];
  dl * dl + da * da + db * db
}

fn update_distances(distances: &mut [f32], color: [f32; 3]) {
  distances
    .par_iter_mut()
    .enumerate()
    .for_each(|(i, d)| {
      *d = d.min(dist_sq(rgb_to_oklab(idx_to_rgb(i)), color));
    });
}

fn argmax(distances: &[f32]) -> usize {
  distances
    .par_iter()
    .enumerate()
    .max_by(|(_, a), (_, b)| a.total_cmp(b))
    .map(|(i, _)| i)
    .unwrap()
}

// Optimal 1-to-1 assignment of palette colors to SOM grid cells.
// Uses the O(n³) Kuhn-Munkres algorithm with dual potentials.
// Workers = grid cells, jobs = palette colors.
// Returns result[grid_pos] = palette_idx.
fn hungarian(weights: &[[f32; 3]], labs: &[[f32; 3]]) -> Vec<usize> {
  let n = labs.len();

  // cost[i][j] = distance between grid cell i's weight and palette color j (1-indexed).
  let cost = |i: usize, j: usize| dist_sq(weights[i - 1], labs[j - 1]);

  let mut u = vec![0.0f32; n + 1]; // row potentials (workers = grid cells)
  let mut v = vec![0.0f32; n + 1]; // col potentials (jobs = palette colors)
  let mut p = vec![0usize; n + 1]; // p[j] = worker assigned to job j
  let mut way = vec![0usize; n + 1];

  for i in 1..=n {
    p[0] = i;
    let mut j0 = 0usize;
    let mut minval = vec![f32::MAX; n + 1];
    let mut used = vec![false; n + 1];

    loop {
      used[j0] = true;
      let i0 = p[j0];
      let mut delta = f32::MAX;
      let mut j1 = 0usize;

      for j in 1..=n {
        if !used[j] {
          let cur = cost(i0, j) - u[i0] - v[j];
          if cur < minval[j] {
            minval[j] = cur;
            way[j] = j0;
          }
          if minval[j] < delta {
            delta = minval[j];
            j1 = j;
          }
        }
      }

      for j in 0..=n {
        if used[j] {
          u[p[j]] += delta;
          v[j] -= delta;
        } else {
          minval[j] -= delta;
        }
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

  // p[j] = i means grid cell i-1 is assigned palette color j-1.
  let mut result = vec![0usize; n];
  for j in 1..=n {
    if p[j] != 0 {
      result[p[j] - 1] = j - 1;
    }
  }
  result
}

// Continuous toroidal SOM. Trains floating-point weight vectors then uses
// Hungarian assignment for an optimal, artifact-free final mapping.
fn som_layout(labs: &[[f32; 3]]) -> Vec<usize> {
  let n = labs.len();
  let side = (n as f32).sqrt() as usize;
  let mut rng = SmallRng::seed_from_u64(42);

  let mut weights: Vec<[f32; 3]> = labs.to_vec();
  weights.shuffle(&mut rng);

  let num_iters  = 200_000usize;
  let sigma_init = side as f32 / 2.0;
  let sigma_final = 0.5f32;
  let lr_init    = 0.5f32;
  let lr_final   = 0.01f32;

  eprintln!("Training toroidal SOM ({num_iters} iterations)...");

  for t in 0..num_iters {
    let progress    = t as f32 / num_iters as f32;
    let sigma       = sigma_init * (sigma_final / sigma_init).powf(progress);
    let lr          = lr_init   * (lr_final   / lr_init  ).powf(progress);
    let two_sig_sq  = 2.0 * sigma * sigma;

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
      eprintln!("  {}/{num_iters} (σ={sigma:.2}, lr={lr:.3})", t + 1);
    }
  }

  eprintln!("Computing optimal assignment (Hungarian algorithm)...");
  hungarian(&weights, labs)
}

pub fn generate(size: usize) -> Vec<[u8; 3]> {
  assert!(size >= SEEDS.len(), "size must be at least {}", SEEDS.len());

  // Glasbey
  let mut palette: Vec<usize> = SEEDS.iter().map(|&rgb| rgb_to_idx(rgb)).collect();
  let mut distances = vec![f32::MAX; NUM_COLORS];

  for &idx in &palette[..palette.len() - 1] {
    update_distances(&mut distances, rgb_to_oklab(idx_to_rgb(idx)));
  }

  while palette.len() < size {
    let last = *palette.last().unwrap();
    update_distances(&mut distances, rgb_to_oklab(idx_to_rgb(last)));
    let next = argmax(&distances);
    eprintln!("[{:3}/{}] #{:06X}", palette.len() + 1, size, next);
    palette.push(next);
  }

  let colors: Vec<[u8; 3]> = palette.iter().map(|&idx| idx_to_rgb(idx)).collect();

  // SOM + Hungarian layout
  let labs: Vec<[f32; 3]> = colors.iter().map(|&rgb| rgb_to_oklab(rgb)).collect();
  let grid = som_layout(&labs);

  // Write image
  let side = (size as f32).sqrt() as usize;
  let path = "palette.png";
  let side_u32 = side as u32;
  let mut img = image::RgbImage::new(side_u32, side_u32);
  for (grid_pos, &palette_idx) in grid.iter().enumerate() {
    let [r, g, b] = colors[palette_idx];
    img.put_pixel((grid_pos % side) as u32, (grid_pos / side) as u32, image::Rgb([r, g, b]));
  }
  img.save(path).expect("failed to save palette.png");

  // Verify
  let saved = image::open(path).expect("failed to re-open palette.png").into_rgb8();
  let mut palette_set: std::collections::HashSet<[u8; 3]> = colors.iter().cloned().collect();
  let mut mismatches = 0;
  for pixel in saved.pixels() {
    let color = [pixel[0], pixel[1], pixel[2]];
    if !palette_set.remove(&color) {
      eprintln!("  MISMATCH: #{:02X}{:02X}{:02X}", color[0], color[1], color[2]);
      mismatches += 1;
    }
  }
  if mismatches == 0 && palette_set.is_empty() {
    eprintln!("Verification passed: all {size} colors present and correct.");
  } else {
    if mismatches > 0 {
      eprintln!("Verification FAILED: {mismatches} pixel(s) not in original palette.");
    }
    if !palette_set.is_empty() {
      eprintln!("Verification FAILED: {} color(s) missing from image.", palette_set.len());
    }
  }

  colors
}