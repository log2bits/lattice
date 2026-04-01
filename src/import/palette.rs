use std::collections::HashMap;

use crate::dag::ColorPalette;
use kmeans::{EuclideanDistance, KMeans, KMeansConfig};
use rayon::prelude::*;
use oklab::{srgb_to_oklab, oklab_to_srgb, Rgb, Oklab};

pub fn build_palette(colors: &[[u8; 3]]) -> [[u8; 3]; 256] {
  let colors_oklab: Vec<f32> = colors
    .par_iter()
    .flat_map(|&rgb| rgb_to_oklab(rgb))
    .collect();

  let (sample_cnt, sample_dims, k, max_iter) = (colors.len(), 3, 256, 100);
  let kmean: KMeans<f32, 8, _> = KMeans::new(&colors_oklab, sample_cnt, sample_dims, EuclideanDistance);
  let result = kmean.kmeans_lloyd(k, max_iter, KMeans::init_kmeanplusplus, &KMeansConfig::default());

  let centroids: Vec<f32> = result.centroids.iter().flatten().copied().collect();
  let mut palette = [[0u8; 3]; 256];
  centroids
    .chunks(3)
    .enumerate()
    .for_each(|(i, lab)| palette[i] = oklab_to_rgb([lab[0], lab[1], lab[2]]));
  palette
}

pub fn build_palette_from_images(images: &[&[[u8; 3]]]) -> [[u8; 3]; 256] {
  let color_counts: HashMap<[u8; 3], usize> = images
    .par_iter()
    .flat_map(|img| img.par_iter().copied())
    .fold(HashMap::new, |mut map, color| {
      *map.entry(color).or_insert(0) += 1;
      map
    })
    .reduce(HashMap::new, |mut a, b| {
      for (color, count) in b {
        *a.entry(color).or_insert(0) += count;
      }
      a
    });

  let max_count = color_counts.values().copied().max().unwrap_or(1);
  const MAX_REPEATS: usize = 100;

  let colors: Vec<[u8; 3]> = color_counts
    .into_iter()
    .flat_map(|(color, count)| {
      // ceil(count / max_count * 100), minimum 1 guaranteed since count >= 1
      let repeats = (count * MAX_REPEATS + max_count - 1) / max_count;
      std::iter::repeat(color).take(repeats)
    })
    .collect();

  build_palette(&colors)
}

fn rgb_to_oklab(rgb: [u8; 3]) -> [f32; 3] {
  let oklab = srgb_to_oklab(Rgb { r: rgb[0], g: rgb[1], b: rgb[2] });
  [oklab.l, oklab.a, oklab.b]
}

fn oklab_to_rgb(lab: [f32; 3]) -> [u8; 3] {
  let rgb = oklab_to_srgb(Oklab { l: lab[0], a: lab[1], b: lab[2] });
  [rgb.r, rgb.g, rgb.b]
}