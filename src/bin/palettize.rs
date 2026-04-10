use lattice::import::palette::rgb_to_oklab;
use rayon::prelude::*;
use std::env;

#[inline(always)]
fn dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
	let dl = a[0] - b[0];
	let da = a[1] - b[1];
	let db = a[2] - b[2];
	dl * dl + da * da + db * db
}

fn main() {
	let args: Vec<String> = env::args().collect();
	if args.len() != 3 {
		eprintln!("Usage: palettize <input_image> <output_image>");
		std::process::exit(1);
	}

	let input_path = &args[1];
	let output_path = &args[2];

	let palette_img = image::open("palette.png").expect("failed to open palette.png").into_rgb8();

	let palette: Vec<[u8; 3]> = palette_img.pixels().map(|p| [p[0], p[1], p[2]]).collect();

	let palette_labs: Vec<[f32; 3]> = palette.iter().map(|&rgb| rgb_to_oklab(rgb)).collect();

	eprintln!("Loaded {} colors from palette.png.", palette.len());

	let input = image::open(input_path).expect("failed to open input image").into_rgba8();

	let (w, h) = input.dimensions();
	eprintln!("Palettizing {}x{} image...", w, h);

	let pixels: Vec<[u8; 4]> = input.pixels().map(|p| [p[0], p[1], p[2], p[3]]).collect();

	let output_pixels: Vec<[u8; 4]> = pixels
		.par_iter()
		.map(|&[r, g, b, a]| {
			let lab = rgb_to_oklab([r, g, b]);
			let best = palette_labs
				.iter()
				.enumerate()
				.min_by(|(_, x), (_, y)| dist_sq(**x, lab).total_cmp(&dist_sq(**y, lab)))
				.map(|(i, _)| i)
				.unwrap();
			let [pr, pg, pb] = palette[best];
			[pr, pg, pb, a]
		})
		.collect();

	let mut output = image::RgbaImage::new(w, h);
	for (i, &[r, g, b, a]) in output_pixels.iter().enumerate() {
		let x = (i as u32) % w;
		let y = (i as u32) / w;
		output.put_pixel(x, y, image::Rgba([r, g, b, a]));
	}

	output.save(output_path).expect("failed to save output image");
	eprintln!("Saved to {output_path}.");
}
