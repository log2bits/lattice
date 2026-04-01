use std::path::Path;
use lattice::import::gltf::import_gltf;

fn main() {
  import_gltf(Path::new("scenes/bistro.glb"));
}