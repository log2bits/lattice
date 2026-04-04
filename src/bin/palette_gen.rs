use palette::{FromColor, IntoColor, Oklab, Oklch, Srgb};
use rayon::prelude::*;
use std::collections::{BinaryHeap, HashMap};

const NUM_COLORS: usize = 256 * 256 * 256; // 16_777_216

// Fixed grid scale used only for exact neighbour lookup acceleration.
// We keep this constant for the entire run to avoid late-stage slowdown from
// increasingly fat cells.
const ALPHA: f32 = 1.5;

// Pin black and white until the very end.
const BLACK_IDX: u32 = 0x000000;
const WHITE_IDX: u32 = 0xFFFFFF;


// Colour helpers


#[inline(always)]
pub fn idx_to_rgb(idx: usize) -> [u8; 3] {
  [(idx >> 16) as u8, (idx >> 8) as u8, idx as u8]
}

#[inline(always)]
pub fn rgb_to_idx(rgb: [u8; 3]) -> usize {
  ((rgb[0] as usize) << 16) | ((rgb[1] as usize) << 8) | rgb[2] as usize
}

#[inline(always)]
fn is_pinned(idx: u32) -> bool {
  idx == BLACK_IDX || idx == WHITE_IDX
}

fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
  let srgb = Srgb::new(rgb[0], rgb[1], rgb[2]).into_format::<f32>();
  let lab: Oklab = srgb.into_color();
  [lab.l, lab.a, lab.b]
}

fn oklab_to_srgb(lab: [f32; 3]) -> [u8; 3] {
  let oklab = Oklab::new(lab[0], lab[1], lab[2]);
  let srgb: Srgb<f32> = Srgb::from_color(oklab);
  let r = (srgb.red * 255.0).round() as i32;
  let g = (srgb.green * 255.0).round() as i32;
  let b = (srgb.blue * 255.0).round() as i32;
  if r >= 0 && r <= 255 && g >= 0 && g <= 255 && b >= 0 && b <= 255 {
    return [r as u8, g as u8, b as u8];
  }

  let lch: Oklch = oklab.into_color();
  let mut lo = 0.0f32;
  let mut hi = lch.chroma;
  for _ in 0..24 {
    let mid = (lo + hi) / 2.0;
    let s: Srgb<f32> =
      Srgb::from_color(Oklab::from_color(Oklch::new(lch.l, mid, lch.hue)));
    if s.red >= 0.0
      && s.red <= 1.0
      && s.green >= 0.0
      && s.green <= 1.0
      && s.blue >= 0.0
      && s.blue <= 1.0
    {
      lo = mid;
    } else {
      hi = mid;
    }
  }

  let s: Srgb<f32> = Srgb::from_color(Oklab::from_color(Oklch::new(lch.l, lo, lch.hue)));
  [
    (s.red * 255.0).round().clamp(0.0, 255.0) as u8,
    (s.green * 255.0).round().clamp(0.0, 255.0) as u8,
    (s.blue * 255.0).round().clamp(0.0, 255.0) as u8,
  ]
}

#[inline(always)]
fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
  let dl = a[0] - b[0];
  let da = a[1] - b[1];
  let db = a[2] - b[2];
  dl * dl + da * da + db * db
}


// Global LUT


static LUT: std::sync::OnceLock<Vec<[f32; 3]>> = std::sync::OnceLock::new();

#[inline(always)]
fn lut_lab(idx: u32) -> [f32; 3] {
  LUT.get().unwrap()[idx as usize]
}


// Grid acceleration structure
//
// Exact nearest / second-nearest neighbour search using expanding L-inf shells.
// The grid only accelerates candidate lookup; all decisions are based on exact
// continuous OKLab distances.


struct Grid {
  cells: HashMap<u64, Vec<u32>>,
  cell_size: f32,
  min_lab: [f32; 3],
  max_cell: [i32; 3],
}

#[inline(always)]
fn pack_key(cx: i32, cy: i32, cz: i32) -> u64 {
  const BIAS: i32 = 1 << 19;
  ((cx + BIAS) as u64) | (((cy + BIAS) as u64) << 20) | (((cz + BIAS) as u64) << 40)
}

impl Grid {
  fn new(cell_size: f32, min_lab: [f32; 3], max_lab: [f32; 3], cap: usize) -> Self {
    let max_cell = [
      ((max_lab[0] - min_lab[0]) / cell_size).floor() as i32,
      ((max_lab[1] - min_lab[1]) / cell_size).floor() as i32,
      ((max_lab[2] - min_lab[2]) / cell_size).floor() as i32,
    ];

    Self {
      cells: HashMap::with_capacity(cap),
      cell_size,
      min_lab,
      max_cell,
    }
  }

  #[inline(always)]
  fn cell_of(&self, lab: [f32; 3]) -> (i32, i32, i32) {
    (
      ((lab[0] - self.min_lab[0]) / self.cell_size).floor() as i32,
      ((lab[1] - self.min_lab[1]) / self.cell_size).floor() as i32,
      ((lab[2] - self.min_lab[2]) / self.cell_size).floor() as i32,
    )
  }

  fn insert(&mut self, idx: u32, lab: [f32; 3]) {
    let (cx, cy, cz) = self.cell_of(lab);
    self.cells.entry(pack_key(cx, cy, cz)).or_default().push(idx);
  }

  fn remove(&mut self, idx: u32, lab: [f32; 3]) {
    let (cx, cy, cz) = self.cell_of(lab);
    let key = pack_key(cx, cy, cz);
    if let Some(v) = self.cells.get_mut(&key) {
      if let Some(pos) = v.iter().position(|&x| x == idx) {
        v.swap_remove(pos);
      }
      if v.is_empty() {
        self.cells.remove(&key);
      }
    }
  }

  // Returns:
  //  (nearest_idx, nearest_dist, second_idx, second_dist)
  // Missing neighbours are reported as u32::MAX / f32::MAX.
  fn find_two_nn(&self, q_lab: [f32; 3], q_idx: u32) -> (u32, f32, u32, f32) {
    let (cx, cy, cz) = self.cell_of(q_lab);

    let mut best1_idx = u32::MAX;
    let mut best1_dsq = f32::MAX;
    let mut best2_idx = u32::MAX;
    let mut best2_dsq = f32::MAX;
    let mut found = 0usize;

    let full_cover_radius = [
      cx,
      self.max_cell[0] - cx,
      cy,
      self.max_cell[1] - cy,
      cz,
      self.max_cell[2] - cz,
    ]
    .into_iter()
    .max()
    .unwrap()
    .max(0);

    let mut s: i32 = 0;

    loop {
      for dz in -s..=s {
        for dy in -s..=s {
          for dx in -s..=s {
            if s > 0 && dx.abs() < s && dy.abs() < s && dz.abs() < s {
              continue;
            }

            let nx = cx + dx;
            let ny = cy + dy;
            let nz = cz + dz;

            if nx < 0
              || ny < 0
              || nz < 0
              || nx > self.max_cell[0]
              || ny > self.max_cell[1]
              || nz > self.max_cell[2]
            {
              continue;
            }

            let key = pack_key(nx, ny, nz);
            if let Some(v) = self.cells.get(&key) {
              for &ni in v {
                if ni == q_idx {
                  continue;
                }

                let d = dist_sq(q_lab, lut_lab(ni));

                if d < best1_dsq || (d == best1_dsq && ni < best1_idx) {
                  best2_dsq = best1_dsq;
                  best2_idx = best1_idx;
                  best1_dsq = d;
                  best1_idx = ni;
                  found = found.max(1);
                  if best2_idx != u32::MAX {
                    found = 2;
                  }
                } else if d < best2_dsq || (d == best2_dsq && ni < best2_idx) {
                  best2_dsq = d;
                  best2_idx = ni;
                  found = found.max(2);
                }
              }
            }
          }
        }
      }

      let min_unvisited = s as f32 * self.cell_size;
      let min_unvisited_dsq = min_unvisited * min_unvisited;

      let done1 = found >= 1 && min_unvisited_dsq >= best1_dsq;
      let done2 = found >= 2 && min_unvisited_dsq >= best2_dsq;

      if done1 && done2 {
        break;
      }

      if s >= full_cover_radius {
        break;
      }

      s += 1;
    }

    (
      best1_idx,
      if best1_dsq == f32::MAX {
        f32::MAX
      } else {
        best1_dsq.sqrt()
      },
      best2_idx,
      if best2_dsq == f32::MAX {
        f32::MAX
      } else {
        best2_dsq.sqrt()
      },
    )
  }
}


// Heap
//
// We always want the point with the smallest d1.
// Tie-break by smaller rgb index.
// BinaryHeap is max-heap, so store complemented keys.


#[derive(Clone, Copy)]
struct HeapEntry {
  neg_d1_bits: u32,
  neg_rgb: u32,
  idx: u32,
  version: u32,
}

impl PartialEq for HeapEntry {
  fn eq(&self, o: &Self) -> bool {
    self.neg_d1_bits == o.neg_d1_bits && self.neg_rgb == o.neg_rgb
  }
}
impl Eq for HeapEntry {}

impl Ord for HeapEntry {
  fn cmp(&self, o: &Self) -> std::cmp::Ordering {
    self.neg_d1_bits
      .cmp(&o.neg_d1_bits)
      .then(self.neg_rgb.cmp(&o.neg_rgb))
  }
}

impl PartialOrd for HeapEntry {
  fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(o))
  }
}

#[inline(always)]
fn make_entry(idx: u32, d1: f32, version: u32) -> HeapEntry {
  HeapEntry {
    neg_d1_bits: !d1.to_bits(),
    neg_rgb: !idx,
    idx,
    version,
  }
}


// Closest-pair elimination with 1st/2nd nearest-neighbour tie-break
//
// Pinned colours (black and white) are never removed during the main loop.
// They are appended manually at the end, in this order:
//  1. black
//  2. white
//
// So white is the final survivor.


pub fn closest_pair_second_nn_elimination() -> Vec<u32> {
  let lut = LUT.get().unwrap();
  let n = NUM_COLORS;

  eprintln!("Measuring OKLab bounding box...");
  let (min_lab, max_lab) = {
    let mn = lut.par_iter().copied().reduce(
      || [f32::MAX; 3],
      |a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
    );
    let mx = lut.par_iter().copied().reduce(
      || [f32::MIN; 3],
      |a, b| [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
    );
    (mn, mx)
  };

  let volume =
    (max_lab[0] - min_lab[0]) * (max_lab[1] - min_lab[1]) * (max_lab[2] - min_lab[2]);

  let cell_size = ALPHA * (volume / n as f32).cbrt();

  eprintln!("Building grid cell_size={cell_size:.6}...");
  let mut grid = {
    let mut g = Grid::new(cell_size, min_lab, max_lab, n / 4);
    for i in 0..n {
      g.insert(i as u32, lut[i]);
    }
    g
  };
  eprintln!(" {} non-empty cells", grid.cells.len());

  eprintln!("Computing initial 1st/2nd nearest neighbours (parallel)...");
  let init: Vec<(u32, f32, u32, f32)> = (0..n as u32)
    .into_par_iter()
    .map(|i| grid.find_two_nn(lut[i as usize], i))
    .collect();

  let mut nn1 = vec![u32::MAX; n];
  let mut d1 = vec![f32::MAX; n];
  let mut nn2 = vec![u32::MAX; n];
  let mut d2 = vec![f32::MAX; n];

  for i in 0..n {
    let (a, b, c, d) = init[i];
    nn1[i] = a;
    d1[i] = b;
    nn2[i] = c;
    d2[i] = d;
  }

  let mut active = vec![true; n];
  let mut version = vec![0u32; n];

  eprintln!("Building reverse dependency lists...");
  // deps[x] = points i such that nn1[i] == x or nn2[i] == x.
  let mut deps: Vec<Vec<u32>> = vec![Vec::new(); n];
  for i in 0..n {
    if nn1[i] != u32::MAX {
      deps[nn1[i] as usize].push(i as u32);
    }
    if nn2[i] != u32::MAX && nn2[i] != nn1[i] {
      deps[nn2[i] as usize].push(i as u32);
    }
  }

  eprintln!("Building heap...");
  let mut heap: BinaryHeap<HeapEntry> = BinaryHeap::with_capacity(n * 2);
  for i in 0..n {
    heap.push(make_entry(i as u32, d1[i], 0));
  }

  let mut order: Vec<u32> = Vec::with_capacity(n);
  let mut remaining = n;
  let report_every = (n / 200).max(1);

  // Used to deduplicate affected points within one deletion step.
  let mut seen = vec![0u32; n];
  let mut stamp = 1u32;

  eprintln!("Starting elimination loop...");

  // Leave the two pinned colours alive until the end.
  while remaining > 2 {
    let a = loop {
      match heap.pop() {
        None => break u32::MAX,
        Some(e) => {
          if !active[e.idx as usize] {
            continue;
          }
          if is_pinned(e.idx) {
            continue;
          }
          if e.version != version[e.idx as usize] {
            continue;
          }
          break e.idx;
        }
      }
    };

    if a == u32::MAX {
      break;
    }

    // If this non-pinned point has no neighbours, remove it directly.
    if nn1[a as usize] == u32::MAX {
      active[a as usize] = false;
      grid.remove(a, lut[a as usize]);
      order.push(a);
      remaining -= 1;
      continue;
    }

    let b = nn1[a as usize];

    // If b is stale, recompute a and continue.
    if b != u32::MAX && !active[b as usize] {
      let (new1, newd1, new2, newd2) = grid.find_two_nn(lut[a as usize], a);

      nn1[a as usize] = new1;
      d1[a as usize] = newd1;
      nn2[a as usize] = new2;
      d2[a as usize] = newd2;

      if new1 != u32::MAX {
        deps[new1 as usize].push(a);
      }
      if new2 != u32::MAX && new2 != new1 {
        deps[new2 as usize].push(a);
      }

      version[a as usize] += 1;
      heap.push(make_entry(a, newd1, version[a as usize]));
      continue;
    }

    // Pinned colours cannot be victims.
    let victim = if b == u32::MAX {
      a
    } else if is_pinned(b) {
      a
    } else {
      let da2 = d2[a as usize];
      let db2 = d2[b as usize];

      if da2 < db2 {
        a
      } else if db2 < da2 {
        b
      } else if a < b {
        a
      } else {
        b
      }
    };

    active[victim as usize] = false;
    grid.remove(victim, lut[victim as usize]);
    order.push(victim);
    remaining -= 1;

    // Collect affected points:
    // - all points that referenced victim as nn1 or nn2
    // - the partner in the closest pair, because its local state changed
    let mut affected = std::mem::take(&mut deps[victim as usize]);

    let partner = if victim == a { b } else { a };
    if partner != u32::MAX && active[partner as usize] {
      affected.push(partner);
    }

    stamp = stamp.wrapping_add(1);
    if stamp == 0 {
      seen.fill(0);
      stamp = 1;
    }

    for q in affected {
      let qi = q as usize;
      if !active[qi] {
        continue;
      }
      if seen[qi] == stamp {
        continue;
      }
      seen[qi] = stamp;

      let (new1, newd1, new2, newd2) = grid.find_two_nn(lut[qi], q);

      nn1[qi] = new1;
      d1[qi] = newd1;
      nn2[qi] = new2;
      d2[qi] = newd2;

      if new1 != u32::MAX {
        deps[new1 as usize].push(q);
      }
      if new2 != u32::MAX && new2 != new1 {
        deps[new2 as usize].push(q);
      }

      version[qi] += 1;
      heap.push(make_entry(q, newd1, version[qi]));
    }

    let done = n - remaining;
    if done % report_every == 0 || remaining == 2 {
      eprintln!(
        " {done}/{n} ({:.1}%) remaining={remaining}",
        done as f64 / n as f64 * 100.0
      );
    }
  }

  // Append pinned colours at the very end.
  if active[BLACK_IDX as usize] {
    active[BLACK_IDX as usize] = false;
    order.push(BLACK_IDX);
    remaining -= 1;
  }
  if active[WHITE_IDX as usize] {
    active[WHITE_IDX as usize] = false;
    order.push(WHITE_IDX);
    remaining -= 1;
  }

  debug_assert_eq!(remaining, 0);
  eprintln!("Elimination complete. {} colours recorded.", order.len());
  order
}


// Write elimination image


fn write_elimination_image(order: &[u32], path: &str) {
  const W: u32 = 4096;
  const H: u32 = 4096;
  assert_eq!(order.len(), (W * H) as usize);
  eprintln!("Writing {W}x{H} elimination image -> {path}...");
  let mut img = image::RgbImage::new(W, H);
  for (k, &idx) in order.iter().enumerate() {
    let [r, g, b] = idx_to_rgb(idx as usize);
    img.put_pixel((k as u32) % W, (k as u32) / W, image::Rgb([r, g, b]));
  }
  img.save(path).expect("failed to save elimination image");
  eprintln!("Saved {path}.");
}


// Palette pipeline


#[inline(always)]
fn rgb_idx_flat(c: [u8; 3]) -> usize {
  ((c[0] as usize) << 16) | ((c[1] as usize) << 8) | c[2] as usize
}

fn hungarian(weights: &[[f32; 3]], labs: &[[f32; 3]]) -> Vec<usize> {
  let n = labs.len();
  let cost = |i: usize, j: usize| dist_sq(weights[i - 1], labs[j - 1]);
  let mut u = vec![0.0f32; n + 1];
  let mut v = vec![0.0f32; n + 1];
  let mut p = vec![0usize; n + 1];
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
      if p[j0] == 0 {
        break;
      }
    }
    loop {
      let j1 = way[j0];
      p[j0] = p[j1];
      j0 = j1;
      if j0 == 0 {
        break;
      }
    }
  }

  let mut result = vec![0usize; n];
  for j in 1..=n {
    if p[j] != 0 {
      result[p[j] - 1] = j - 1;
    }
  }
  result
}

fn som_layout(labs: &[[f32; 3]]) -> Vec<usize> {
  use rand::prelude::*;

  let n = labs.len();
  let side = (n as f32).sqrt() as usize;
  let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
  let mut weights: Vec<[f32; 3]> = labs.to_vec();
  weights.shuffle(&mut rng);

  let num_iters = 20000usize;
  let sigma_init = side as f32 / 2.0;
  let sigma_final = 0.1f32;
  let lr_init = 1.0f32;
  let lr_final = 0.001f32;

  eprintln!("Training toroidal SOM ({num_iters} iterations)...");
  for t in 0..num_iters {
    let progress = t as f32 / num_iters as f32;
    let sigma = sigma_init * (sigma_final / sigma_init).powf(progress);
    let lr = lr_init * (lr_final / lr_init).powf(progress);
    let two_sig_sq = 2.0 * sigma * sigma;
    let input = labs[rng.gen_range(0..n)];
    let bmu = (0..n)
      .min_by(|&a, &b| dist_sq(weights[a], input).total_cmp(&dist_sq(weights[b], input)))
      .unwrap();

    let bmu_row = (bmu / side) as f32;
    let bmu_col = (bmu % side) as f32;

    for j in 0..n {
      let dr = {
        let d = ((j / side) as f32 - bmu_row).abs();
        d.min(side as f32 - d)
      };
      let dc = {
        let d = ((j % side) as f32 - bmu_col).abs();
        d.min(side as f32 - d)
      };
      let influence = (-(dr * dr + dc * dc) / two_sig_sq).exp();
      if influence < 1e-6 {
        continue;
      }
      for k in 0..3 {
        weights[j][k] += lr * influence * (input[k] - weights[j][k]);
      }
    }

    if (t + 1) % 50_000 == 0 {
      eprintln!(" {}/{num_iters} (sigma={sigma:.2}, lr={lr:.3})", t + 1);
    }
  }

  eprintln!("Computing optimal assignment (Hungarian)...");
  let grid = hungarian(&weights, labs);

  let darkest = labs
    .iter()
    .enumerate()
    .min_by(|(_, a), (_, b)| a[0].total_cmp(&b[0]))
    .map(|(i, _)| i)
    .unwrap();

  let dark_pos = grid.iter().position(|&p| p == darkest).unwrap();
  let sr = dark_pos / side;
  let sc = dark_pos % side;

  let mut shifted = vec![0usize; n];
  for pos in 0..n {
    let new_row = (pos / side + side - sr) % side;
    let new_col = (pos % side + side - sc) % side;
    shifted[new_row * side + new_col] = grid[pos];
  }
  shifted
}

fn generate_hald_clut(palette_labs: &[[f32; 3]], palette_rgb: &[[u8; 3]]) {
  const LEVEL: usize = 8;
  const STEPS: usize = LEVEL * LEVEL;
  const IMG_SIZE: usize = LEVEL * LEVEL * LEVEL;

  eprintln!("Generating Hald CLUT ({IMG_SIZE}x{IMG_SIZE})...");
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
            .min_by(|(_, a), (_, b)| {
              dist_sq(**a, lab).total_cmp(&dist_sq(**b, lab))
            })
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
  img.save("assets/colors/palette_lut.png")
    .expect("failed to save palette_lut.png");
  eprintln!("Saved palette_lut.png.");
}

pub fn generate(size: usize, order: &[u32]) -> Vec<[u8; 3]> {
  let lut = LUT.get().unwrap();
  let side = (size as f32).sqrt() as usize;
  assert_eq!(side * side, size, "size must be a perfect square");

  let colors: Vec<[u8; 3]> = order[order.len() - size..]
    .iter()
    .map(|&idx| idx_to_rgb(idx as usize))
    .collect();

  let mut taken: std::collections::HashSet<[u8; 3]> = std::collections::HashSet::new();
  let mut colors = colors;
  let mut dupes = 0;

  for i in 0..colors.len() {
    if !taken.insert(colors[i]) {
      dupes += 1;
      let lab = lut[rgb_idx_flat(colors[i])];
      let taken_snap = taken.clone();
      colors[i] = lut
        .par_iter()
        .enumerate()
        .map(|(j, &l)| {
          let rgb = idx_to_rgb(j);
          let penalty = if taken_snap.contains(&rgb) {
            f32::MAX
          } else {
            dist_sq(l, lab)
          };
          (penalty, rgb)
        })
        .min_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, rgb)| rgb)
        .unwrap();
      taken.insert(colors[i]);
    }
  }

  if dupes > 0 {
    eprintln!("Resolved {dupes} duplicate(s).");
  }

  let labs: Vec<[f32; 3]> = colors.iter().map(|&rgb| lut[rgb_idx_flat(rgb)]).collect();
  let layout = som_layout(&labs);

  let path = format!("assets/colors/palette_{}.png", size);
  let mut img = image::RgbImage::new(side as u32, side as u32);
  for (pos, &idx) in layout.iter().enumerate() {
    let [r, g, b] = colors[idx];
    img.put_pixel((pos % side) as u32, (pos / side) as u32, image::Rgb([r, g, b]));
  }
  img.save(&path).expect("failed to save palette.png");

  let saved = image::open(path)
    .expect("failed to re-open palette.png")
    .into_rgb8();
  let mut set: std::collections::HashSet<[u8; 3]> = colors.iter().cloned().collect();
  let mut mismatches = 0;
  for pixel in saved.pixels() {
    let c = [pixel[0], pixel[1], pixel[2]];
    if !set.remove(&c) {
      eprintln!(" MISMATCH: #{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
      mismatches += 1;
    }
  }

  if mismatches == 0 && set.is_empty() {
    eprintln!("Verification passed: all {size} colours present and correct.");
  } else {
    if mismatches > 0 {
      eprintln!(
        "Verification FAILED: {mismatches} pixel(s) not in original palette."
      );
    }
    if !set.is_empty() {
      eprintln!("Verification FAILED: {} colour(s) missing.", set.len());
    }
  }

  generate_hald_clut(&labs, &colors);
  colors
}


// Main
//
// Modes:
//  elim      Run elimination -> write elimination.png
//  palette <N>   Load elimination.png -> extract last N colours -> layout
//  all [N]     Run elimination then palette (default N=256)


fn main() {
  let args: Vec<String> = std::env::args().collect();
  let mode = args.get(1).map(|s| s.as_str()).unwrap_or("all");

  eprintln!("Building OKLab LUT ({NUM_COLORS} colours)...");
  let lut: Vec<[f32; 3]> = (0..NUM_COLORS)
    .into_par_iter()
    .map(|i| rgb_to_oklab(idx_to_rgb(i)))
    .collect();
  eprintln!(
    "LUT ready (~{}MB).",
    std::mem::size_of_val(lut.as_slice()) / 1_000_000
  );
  LUT.set(lut).ok();

  match mode {
    "elim" => {
      let order = closest_pair_second_nn_elimination();
      write_elimination_image(&order, "assets/colors/palette_generator.png");
    }

    "palette" => {
      let n: usize = args
        .get(2)
        .expect("usage: palette_gen palette <N>")
        .parse()
        .expect("N must be a positive integer");

      eprintln!("Loading palette_generator.png...");
      let img = image::open("assets/colors/palette_generator.png")
        .expect("assets/colors/palette_generator.png not found -- run 'elim' first")
        .into_rgb8();

      let order: Vec<u32> = img
        .pixels()
        .map(|p| rgb_to_idx([p[0], p[1], p[2]]) as u32)
        .collect();

      generate(n, &order);
    }

    "all" => {
      let n: usize = args
        .get(2)
        .unwrap_or(&"256".to_string())
        .parse()
        .expect("N must be a positive integer");

      let order = closest_pair_second_nn_elimination();
      write_elimination_image(&order, "assets/colors/palette_generator.png");
      generate(n, &order);
    }

    _ => {
      eprintln!("usage: palette_gen [elim | palette <N> | all [N]]");
      std::process::exit(1);
    }
  }
}