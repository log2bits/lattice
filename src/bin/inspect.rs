use std::path::PathBuf;
use std::fs::File;
use std::io::BufReader;
use lattice::load::header::parse_header;

fn main() -> Result<(), anyhow::Error> {
  let args: Vec<String> = std::env::args().collect();
  if args.len() != 2 {
    eprintln!("usage: inspect <scene.lattice>");
    std::process::exit(1);
  }

  let input = PathBuf::from(&args[1]);
  let mut reader = BufReader::new(File::open(&input)?);
  let header = parse_header(&mut reader)?;

  println!("version:    {}", header.version);
  println!("world_min:  {:?}", header.world_min);
  println!("world_max:  {:?}", header.world_max);
  println!("voxel_bits: {}", header.voxel_bits);
  println!("sections:   {}", header.sections.len());
  println!("levels:     {}", header.levels.len());
  println!("chunks:     {}", header.chunks.len());

  for (i, sec) in header.sections.iter().enumerate() {
    let layer = match sec.layer_type {
      0 => "Grid",
      1 => "GeometryDag",
      2 => "MaterialDag",
      _ => "Unknown",
    };
    println!("  section {i}: {layer} x{} lut={}", sec.num_levels, sec.lut_enabled != 0);
  }

  Ok(())
}
