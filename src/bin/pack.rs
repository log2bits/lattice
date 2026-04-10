use lattice::format::write::write_lattice;
use lattice::import::gltf::scene_bounds;
use lattice::import::ImportConfig;
use lattice::pack::{pack, PackConfig};
use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
	let args: Vec<String> = std::env::args().collect();

	let mut input_arg: Option<&str> = None;
	let mut output_arg: Option<&str> = None;
	let mut depth: u8 = 4;
	let mut voxel_size: f32 = 0.1;

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
			"-o" => {
				i += 1;
				output_arg = args.get(i).map(|s| s.as_str());
			}
			arg if !arg.starts_with('-') && input_arg.is_none() => input_arg = Some(arg),
			arg => {
				eprintln!("unknown argument: {arg}");
				eprintln!("usage: pack <scene.gltf> -o <out.lattice> [--depth <n>] [--voxel-size <m>]");
				std::process::exit(1);
			}
		}
		i += 1;
	}

	let (Some(input_arg), Some(output_arg)) = (input_arg, output_arg) else {
		eprintln!("usage: pack <scene.gltf> -o <out.lattice> [--depth <n>] [--voxel-size <m>]");
		std::process::exit(1);
	};

	let input = PathBuf::from(input_arg);
	let output = PathBuf::from(output_arg);

	let (world_min, world_max) = scene_bounds(&input)?;
	eprintln!("scene bounds: {world_min:?} -> {world_max:?}");

	let import_config = ImportConfig {
		voxel_size,
		depth,
		palette_path: PathBuf::from("assets/colors/palette_256.png"),
	};

	let pack_config = PackConfig { depth, voxel_size };

	let mut packer = pack(pack_config, &output)?;
	lattice::import::import(&input, &import_config, |chunk_idx, samples| {
		packer.add_chunk(chunk_idx, samples).expect("packer error");
	})?;
	packer.finish()?;

	eprintln!("written to {}", output.display());
	Ok(())
}
