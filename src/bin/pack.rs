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
	};

	let samples = import_gltf(&input, &import_config)?;

	let pack_config = PackConfig {
		dag_depth: 3,
		world_min: import_config.world_min,
		world_max: import_config.world_max,
	};

	pack(pack_config, samples, &output)?;
	eprintln!("Written to {}", output.display());
	Ok(())
}
