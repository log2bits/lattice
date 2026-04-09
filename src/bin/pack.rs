use lattice::import::ImportConfig;
use lattice::import::gltf::{gltf_scene_bounds, import_gltf};
use lattice::pack::{PackConfig, pack};
use std::path::PathBuf;

fn main() -> Result<(), anyhow::Error> {
	let args: Vec<String> = std::env::args().collect();

	let mut input_arg: Option<&str> = None;
	let mut output_arg: Option<&str> = None;
	let mut voxels_per_meter: f64 = 16.0;

	let mut i = 1;
	while i < args.len() {
		match args[i].as_str() {
			"--voxels-per-meter" => {
				i += 1;
				voxels_per_meter = args.get(i)
					.and_then(|s| s.parse().ok())
					.ok_or_else(|| anyhow::anyhow!("--voxels-per-meter requires a positive number"))?;
			}
			arg if !arg.starts_with("--") && input_arg.is_none() => input_arg = Some(arg),
			arg if !arg.starts_with("--") && output_arg.is_none() => output_arg = Some(arg),
			arg => {
				eprintln!("unknown argument: {}", arg);
				eprintln!("usage: pack <scene.gltf> <out.lattice> [--voxels-per-meter <n>]");
				std::process::exit(1);
			}
		}
		i += 1;
	}

	let (Some(input_arg), Some(output_arg)) = (input_arg, output_arg) else {
		eprintln!("usage: pack <scene.gltf> <out.lattice> [--voxels-per-meter <n>]");
		std::process::exit(1);
	};

	let input = PathBuf::from(input_arg);
	let output = PathBuf::from(output_arg);

	let voxel_size = 1.0 / voxels_per_meter;
	let (world_min, world_max) = gltf_scene_bounds(&input, voxel_size)?;
	eprintln!("scene bounds: {:?} to {:?} (voxels)", world_min, world_max);

	let import_config = ImportConfig {
		voxel_size,
		world_min,
		world_max,
		chunk_size: 64,
		palette_path: std::path::PathBuf::from("assets/colors/palette_256.png"),
	};

	let pack_config = PackConfig {
		depth: 3,
		world_min,
		world_max,
	};

	let mut packer = pack(pack_config, &output)?;
	import_gltf(&input, &import_config, |chunk| packer.add_chunk(chunk))?;
	packer.finish()?;
	eprintln!("Written to {}", output.display());
	Ok(())
}
