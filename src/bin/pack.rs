use lattice::import::ImportConfig;
use lattice::import::gltf::import_gltf;
use lattice::pack::{PackConfig, pack};
use std::path::PathBuf;

fn main() -> Result<(), anyhow::Error> {
	let args: Vec<String> = std::env::args().collect();
	if args.len() != 3 {
		eprintln!("usage: pack <scene.gltf> <out.lattice>");
		std::process::exit(1);
	}

	let input = PathBuf::from(&args[1]);
	let output = PathBuf::from(&args[2]);

	let import_config = ImportConfig {
		voxel_size: 0.01,
		world_min: [-1024, -1024, -1024],
		world_max: [1024, 1024, 1024],
		chunk_size: 64,
		palette_path: std::path::PathBuf::from("assets/colors/palette_256.png"),
	};

	let pack_config = PackConfig {
		depth: 3,
		world_min: import_config.world_min,
		world_max: import_config.world_max,
	};

	let mut packer = pack(pack_config, &output)?;
	import_gltf(&input, &import_config, |chunk| packer.add_chunk(chunk))?;
	packer.finish()?;
	eprintln!("Written to {}", output.display());
	Ok(())
}
