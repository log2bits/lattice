use crate::dag::ColorPalette;
use rayon::prelude::*;
use oklab::{srgb_to_oklab, oklab_to_srgb, Rgb, Oklab};

fn build_palette(colors: &[[u8; 3]]) -> ColorPalette {
  let colors_oklab: Vec<f32> = colors
    .par_iter()
    .flat_map(|&rgb| rgb_to_oklab(rgb))
    .collect();

  let (sample_cnt, sample_dims, k, max_iter) = (colors.len(), 3, 4, 100);
  let kmean: KMeans<f32, 8, _> = KMeans::new(&colors_oklab, sample_cnt, sample_dims, EuclideanDistance);
  let result = kmean.kmeans_lloyd(k, max_iter, KMeans::init_kmeanplusplus, &KMeansConfig::default());

  println!("Centroids: {:?}", result.centroids);
  println!("Cluster-Assignments: {:?}", result.assignments);
  println!("Error: {}", result.distsum);

  let palette: Vec<[u8; 3]> = result.centroids
    .chunks(3)
    .map(|lab| oklab_to_rgb([lab[0], lab[1], lab[2]]))
    .collect();

  ColorPalette::from(palette)
}

fn build_palette_from_images(images: &[&[[u8; 3]]]) -> ColorPalette {
  let colors: Vec<[u8; 3]> = images.par_iter().flat_map(|img| img.par_iter().copied()).collect();
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