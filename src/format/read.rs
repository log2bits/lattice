use std::io::Read;

use crate::tree::Lattice;

/// Deserialize a Lattice from a .lattice file.
pub fn read_lattice(r: &mut impl Read) -> anyhow::Result<Lattice> {
	todo!()
}
