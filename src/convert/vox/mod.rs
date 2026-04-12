pub mod serialization;
pub mod voxelization;

use crate::convert::io::SceneWriter;
use crate::convert::scene::Scene;
use crate::convert::{Format, OutputFormat, VoxelizationConfig};
use glam::Vec3;
use std::fmt;
use std::io;

/// Determines the algorithm that assigns color indices to
/// generated voxel colors.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ColorMode {
	/// The palette will be static, it will use the default palette defined
	/// by this crate (NOT the default magicavoxel palette!)
	Static
}

impl fmt::Display for ColorMode {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Static => f.write_str("static"),
		}
	}
}

fn voxelize_and_write(
	scene: Scene,
	format_config: &DotVoxConfig,
	voxelization_config: &VoxelizationConfig,
	output: impl SceneWriter,
) -> io::Result<()> {
	let largest_dim = Vec3::from_array(scene.bounds.size()).max_element();
	let scale = voxelization_config.res as f32 / largest_dim;

	let voxel_bounds_size = Vec3::from_array(scene.bounds.size()) * scale;

	let center_offset = -(voxel_bounds_size / 2.0).round().as_ivec3() + 128;

	match format_config.color {
		ColorMode::Static => {
			let data = voxelization::voxelize(
				&scene,
				voxelization_config.res,
				voxelization_config.mode,
				!format_config.no_optimization,
			);

			serialization::write_vox_static(data, output, center_offset)?;
		}
	}

	Ok(())
}

/// Config for the [`DotVox`] voxelizer.
#[derive(Debug, clap::Args)]
#[command(next_help_heading = "`.vox` format options")]
pub struct DotVoxConfig {
	/// The palette generation mode. Dynamic palette looks
	/// much better, but the static palette is much faster.
	///
	/// Dynamic palette is only enabled if the feature `dynamic_palette`
	/// is enabled (the feature is enabled by default)
	#[arg(long, default_value_t = ColorMode::Static)]
	pub color: ColorMode,

	/// With this option, if two triangles share a voxel,
	/// both voxels will be present in the output file
	/// (magicavoxel will likely present the last one)
	#[arg(long, default_value_t = false)]
	pub no_optimization: bool,
}

/// The definition of the output format.
///
/// NOTE: Does not use [`SceneWriter::base_path`]
/// at all. You may return [`None`].
pub struct DotVox;

impl Format for DotVox {
	// Z: up, Y: forward, X: right
	const BASIS: [[f32; 4]; 4] = [
		[1.0, 0.0, 0.0, 0.0],
		[0.0, 0.0, 1.0, 0.0],
		[0.0, 1.0, 0.0, 0.0],
		[0.0, 0.0, 0.0, 1.0],
	];
}

impl OutputFormat for DotVox {
	type Config = DotVoxConfig;
	type Error = io::Error;

	fn voxelize_and_write<W: SceneWriter>(
		scene: Scene,
		output: W,
		format_config: Self::Config,
		voxelization_config: &VoxelizationConfig,
	) -> io::Result<()> {
		voxelize_and_write(scene, &format_config, voxelization_config, output)
	}
}
