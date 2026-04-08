use lattice::load::header::parse_header;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

fn main() -> Result<(), anyhow::Error> {
	let args: Vec<String> = std::env::args().collect();
	if args.len() != 2 {
		eprintln!("usage: inspect <scene.lattice>");
		std::process::exit(1);
	}

	let input = PathBuf::from(&args[1]);
	let mut reader = BufReader::new(File::open(&input)?);
	let header = parse_header(&mut reader)?;

	println!("version:     {}", header.version);
	println!("world_min:   {:?}", header.world_min);
	println!("world_max:   {:?}", header.world_max);
	println!("depth:       {}", header.depth);
	println!("chunk_count: {}", header.chunk_count);
	println!("levels:      {}", header.levels.len());

	for (i, lvl) in header.levels.iter().enumerate() {
		println!("  level {i}: {} nodes, {}-bit children", lvl.node_count, lvl.child_bits);
	}

	Ok(())
}
