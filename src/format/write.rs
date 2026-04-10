use std::io::Write;

use crate::tree::Lattice;

/// Serialize a Lattice to the .lattice format.
/// Chunks are written top-down (coarsest depth first) so partial reads can stop early.
pub fn write_lattice(lattice: &Lattice, w: &mut impl Write) -> anyhow::Result<()> {
	todo!()
}
