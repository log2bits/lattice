use std::path::PathBuf;
use lattice::load::load;
use lattice::render::{Renderer, Camera};

fn main() -> Result<(), anyhow::Error> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() != 2 {
    eprintln!("usage: render <scene.lattice>");
    std::process::exit(1);
  }

  let input = PathBuf::from(&args[1]);

  // Window and wgpu setup, then load and render.
  todo!()
}
