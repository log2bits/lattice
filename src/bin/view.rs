use lattice::format::read::read_lattice;
use lattice::render::{Camera, Renderer};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
	let args: Vec<String> = std::env::args().collect();
	if args.len() != 2 {
		eprintln!("usage: view <scene.lattice>");
		std::process::exit(1);
	}

	let input = PathBuf::from(&args[1]);
	let lattice = read_lattice(&mut BufReader::new(File::open(&input)?))?;

	// Window and wgpu setup, then render loop.
	todo!()
}
