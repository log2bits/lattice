use std::path::Path;
use gltf;

use crate::import::palette;

pub mod mesh;
pub mod material;
pub mod voxelizer;

pub fn import_gltf(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
  print!("Loading GLTF...");
  let (document, _buffers, images) = gltf::import(path)?;
  println!("Done.");
  let image_pixels: Vec<Vec<[u8; 3]>> = document
    .textures()
    .filter_map(|texture| {
      let image_data = images.get(texture.source().index())?;
      let pixels = image_data.pixels
        .chunks_exact(bytes_per_pixel(image_data.format))
        .map(|c| [c[0], c[1], c[2]])
        .collect();
      Some(pixels)
    })
    .collect();

  let image_slices: Vec<&[[u8; 3]]> = image_pixels.iter().map(Vec::as_slice).collect();
  
  println!("Collected all image textures, building palette...");

  println!("{:?}", palette::build_palette_from_images(&image_slices));
  Ok(())
}


fn bytes_per_pixel(format: gltf::image::Format) -> usize {
  match format {
    gltf::image::Format::R8 => 1,
    gltf::image::Format::R8G8 => 2,
    gltf::image::Format::R8G8B8 => 3,
    gltf::image::Format::R8G8B8A8 => 4,
    gltf::image::Format::R16 => 2,
    gltf::image::Format::R16G16 => 4,
    gltf::image::Format::R16G16B16 => 6,
    gltf::image::Format::R16G16B16A16 => 8,
    gltf::image::Format::R32G32B32FLOAT => 12,
    gltf::image::Format::R32G32B32A32FLOAT => 16,
    _ => 3
  }
}