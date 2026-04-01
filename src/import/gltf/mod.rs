use std::path::Path;
use gltf;

mod mesh;
mod material;
mod voxelizer;

fn import_gltf(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
  let (document, buffers, images) = gltf::import("scenes/bistro.glb")?;
  
  for texture in document.textures() {
    let name = texture.name().unwrap_or("unnamed");
    println!("Texture: {}", name);

    let image = texture.source();
    if let Some(image_data) = images.get(image.index()) {
      let bpp = bytes_per_pixel(image_data.format);
      
    }
  }

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