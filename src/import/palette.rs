use crate::lattice::ColorPalette;

// Loads the precomputed 256-entry OKLab palette from the palette PNG on disk.
pub fn load_palette(path: &str) -> ColorPalette {
	todo!()
}

// Maps a linear RGB color to the nearest palette entry index using OKLab distance.
pub fn nearest_palette_entry(palette: &ColorPalette, rgb: [u8; 3]) -> u8 {
	todo!()
}
