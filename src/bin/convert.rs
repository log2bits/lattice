use anyhow::{Context as _, Result, bail};
use clap::Parser;
use glam::Mat4;
use lattice::convert::Format;
use lattice::convert::InputFormat;
use lattice::convert::OutputFormat;
use lattice::convert::VoxelizationConfig;
use lattice::convert::gltf::{Gltf, GltfConfig};
use lattice::convert::vox::{DotVox, DotVoxConfig};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn get_extension(path: &Path) -> Result<&str> {
	path.extension()
		.context("failed to verify the file extension")?
		.to_str()
		.context("failed to convert file extension to str")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum InputType {
	GlbGltf,
}

impl InputType {
	pub fn from_file(file: &Path) -> Result<Self> {
		let extension = get_extension(file)?;

		match extension {
			"gltf" | "glb" => Ok(Self::GlbGltf),
			_ => bail!("unknown file extension (only `.gltf` and `.glb` are supported)"),
		}
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputType {
	MagicaVoxel,
	Lattice,
}

impl OutputType {
	pub fn from_file(file: &Path) -> Result<Self> {
		let extension = get_extension(file)?;

		match extension {
			"vox" => Ok(Self::MagicaVoxel),
			"lattice" => Ok(Self::Lattice),
			_ => bail!("unknown file extension (only `.vox` and `.lattice` are supported)"),
		}
	}
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliConfig {
	/// Input file
	#[arg(short, long)]
	pub input: PathBuf,

	/// Output file
	#[arg(short, long)]
	pub output: PathBuf,

	#[command(flatten)]
	pub voxel_cfg: VoxelizationConfig,

	#[command(flatten)]
	pub dotvox_cfg: DotVoxConfig,

	#[command(flatten)]
	pub gltf_cfg: GltfConfig,
}

fn main() -> Result<()> {
	let config = CliConfig::parse();

	let input_format = InputType::from_file(&config.input)?;
	let output_format = OutputType::from_file(&config.output)?;

	let input_basis = match input_format {
		InputType::GlbGltf => Mat4::from_cols_array_2d(&Gltf::BASIS),
	};

	let output_basis_inverse = match output_format {
		OutputType::MagicaVoxel => Mat4::from_cols_array_2d(&DotVox::BASIS).inverse(),
		OutputType::Lattice => Mat4::from_cols_array_2d(&DotVox::BASIS).inverse(),
	};

	let transform_matrix = output_basis_inverse * input_basis;

	let t0 = Instant::now();

	let scene = match input_format {
		InputType::GlbGltf => Gltf::load(
			transform_matrix.to_cols_array_2d(),
			&config.input,
			config.gltf_cfg,
		)?,
	};

	let t1 = Instant::now();

	println!("Scene loaded in {}s", (t1 - t0).as_secs_f32());

	match output_format {
		OutputType::MagicaVoxel => {
			DotVox::voxelize_and_save(scene, &config.output, config.dotvox_cfg, &config.voxel_cfg)?;
		}
		OutputType::Lattice => {}
	}

	let t2 = Instant::now();
	println!("Scene converted and saved in {}s", (t2 - t1).as_secs_f32());

	Ok(())
}
