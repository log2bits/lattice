use lattice::import::{ImportConfig, ImportInfo, VoxelSample};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
  let args: Vec<String> = std::env::args().collect();

  let mut input_arg: Option<&str> = None;
  let mut output_arg: Option<&str> = None;
  let mut depth: u8 = 4;
  let mut voxel_size: f32 = 0.1;
  let mut palette_arg: Option<&str> = None;

  let mut i = 1;
  while i < args.len() {
    match args[i].as_str() {
      "--depth" => {
        i += 1;
        depth = args.get(i)
          .and_then(|s| s.parse().ok())
          .ok_or_else(|| anyhow::anyhow!("--depth requires an integer"))?;
      }
      "--voxel-size" => {
        i += 1;
        voxel_size = args.get(i)
          .and_then(|s| s.parse().ok())
          .ok_or_else(|| anyhow::anyhow!("--voxel-size requires a float"))?;
      }
      "--palette" => {
        i += 1;
        palette_arg = args.get(i).map(|s| s.as_str());
      }
      "-o" => {
        i += 1;
        output_arg = args.get(i).map(|s| s.as_str());
      }
      arg if !arg.starts_with('-') && input_arg.is_none() => input_arg = Some(arg),
      arg => {
        eprintln!("unknown argument: {arg}");
        print_usage();
        std::process::exit(1);
      }
    }
    i += 1;
  }

  let (Some(input_arg), Some(output_arg)) = (input_arg, output_arg) else {
    print_usage();
    std::process::exit(1);
  };

  let input  = PathBuf::from(input_arg);
  let output = PathBuf::from(output_arg);
  let ext = output.extension().and_then(|e| e.to_str()).unwrap_or("");

  let palette_path = palette_arg
    .map(PathBuf::from)
    .or_else(|| {
      // Default palette if the file exists
      let p = PathBuf::from("lattice/assets/colors/palette_256.png");
      p.exists().then_some(p)
    });

  if let Some(ref p) = palette_path {
    eprintln!("palette: {}", p.display());
  } else {
    eprintln!("palette: none (full RGB)");
  }

  let import_config = ImportConfig { voxel_size, depth, palette_path };

  match ext {
    "vox" => export_vox(&input, &output, &import_config)?,
    "lattice" => {
      eprintln!(".lattice export not yet implemented (tree packing is todo)");
      std::process::exit(1);
    }
    _ => {
      eprintln!("unknown output format '{ext}' — use .vox or .lattice");
      std::process::exit(1);
    }
  }

  eprintln!("written to {}", output.display());
  Ok(())
}

fn export_vox(
  input: &std::path::Path,
  output: &std::path::Path,
  config: &ImportConfig,
) -> anyhow::Result<()> {
  let mut chunks: Vec<(u64, Vec<VoxelSample>)> = Vec::new();

  let info: ImportInfo = lattice::import::import(input, config, |idx, samples| {
    eprintln!("  chunk {idx}: {} voxels", samples.len());
    chunks.push((idx, samples));
  })?;

  eprintln!(
    "scene bounds: {:?} -> {:?}  grid: {:?}  chunk_voxels: {}",
    info.world_min, info.world_max, info.grid_dims, info.chunk_voxels
  );

  // Build palette list from loaded palette if one was provided
  let pal_colors: Option<Vec<[u8; 3]>> = config.palette_path.as_ref().map(|p| {
    let pal = lattice::import::palette::Palette::load_palette(p);
    pal.entries.clone()
  });

  let file = std::fs::File::create(output)?;
  let mut writer = std::io::BufWriter::new(file);

  lattice::format::vox::write_vox(
    &mut writer,
    chunks,
    &info,
    pal_colors.as_deref(),
  )?;

  Ok(())
}

fn print_usage() {
  eprintln!("usage: pack <scene.gltf|glb> -o <out.vox|out.lattice>");
  eprintln!("       [--depth <n>] [--voxel-size <m>] [--palette <palette.png>]");
}
