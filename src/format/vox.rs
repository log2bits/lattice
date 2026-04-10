/// Write voxel data from the import pipeline to a MagicaVoxel .vox file.
///
/// Uses a static 6-7-6 RGB palette (252 entries), same approach as voxquant.
/// Good enough for visual verification; no extra quantization dep needed.
use crate::import::{ImportInfo, VoxelSample};
use crate::import::voxelize::morton_decode;
use dot_vox::{Color, DotVoxData, Frame, Model, SceneNode, ShapeModel, Size};
use std::collections::HashMap;
use std::io::{Seek, Write};

// ---- static 6-7-6 palette ---------------------------------------------------

const R_STEPS: u16 = 6;
const G_STEPS: u16 = 7;
const B_STEPS: u16 = 6;

const fn encode_static(color: [u8; 3]) -> u8 {
  let r = color[0] as u16;
  let g = color[1] as u16;
  let b = color[2] as u16;
  let ri = (r * (R_STEPS - 1) + 127) / 255;
  let gi = (g * (G_STEPS - 1) + 127) / 255;
  let bi = (b * (B_STEPS - 1) + 127) / 255;
  (ri + gi * R_STEPS + bi * R_STEPS * G_STEPS) as u8
}

const fn decode_static(byte: u8) -> [u8; 3] {
  if byte == 0 { return [0, 0, 0]; }
  let v = byte as u16;
  let ri = v % R_STEPS;
  let gi = (v / R_STEPS) % G_STEPS;
  let bi = (v / (R_STEPS * G_STEPS)) % B_STEPS;
  [
    (ri * 255 / (R_STEPS - 1)) as u8,
    (gi * 255 / (G_STEPS - 1)) as u8,
    (bi * 255 / (B_STEPS - 1)) as u8,
  ]
}

fn static_palette() -> Vec<Color> {
  (0..=255u8).map(|i| {
    let [r, g, b] = decode_static(i);
    Color { r, g, b, a: 255 }
  }).collect()
}

// ---- vox chunk grouping -----------------------------------------------------

/// MagicaVoxel models are at most 256^3. We group voxels by 256-voxel chunks.
struct VoxChunk {
  origin: [i32; 3],
  voxels: Vec<dot_vox::Voxel>,
}

fn build_vox_chunks(
  all_chunks: Vec<(u64, Vec<VoxelSample>)>,
  info: &ImportInfo,
  palette: Option<&[Color]>,
) -> Vec<VoxChunk> {
  // Collect every voxel as a global position + palette index
  // Global voxel pos = chunk_grid_pos * chunk_voxels + local_pos
  let mut by_vox_chunk: HashMap<[i32; 3], Vec<dot_vox::Voxel>> = HashMap::new();

  let cv = info.chunk_voxels;
  let dims = info.grid_dims;

  for (flat_idx, samples) in all_chunks {
    let cx = (flat_idx as u32) % dims[0];
    let cy = ((flat_idx as u32) / dims[0]) % dims[1];
    let cz = (flat_idx as u32) / (dims[0] * dims[1]);
    let origin = [cx * cv, cy * cv, cz * cv];

    for s in samples {
      let [lx, ly, lz] = morton_decode(s.morton).map(|v| v as i32);
      let gx = origin[0] as i32 + lx;
      let gy = origin[1] as i32 + ly;
      let gz = origin[2] as i32 + lz;

      // Which 256^3 vox chunk does this fall into?
      let vcx = gx.div_euclid(256);
      let vcy = gy.div_euclid(256);
      let vcz = gz.div_euclid(256);
      let vc_key = [vcx, vcy, vcz];

      let lx2 = gx.rem_euclid(256) as u8;
      let ly2 = gy.rem_euclid(256) as u8;
      let lz2 = gz.rem_euclid(256) as u8;

      let rgb = s.voxel.rgb();
      let color_idx = if let Some(pal) = palette {
        // Find closest palette entry (palette is small, linear scan is fine)
        pal.iter().enumerate().skip(1)
          .min_by_key(|(_, c)| {
            let dr = rgb[0] as i32 - c.r as i32;
            let dg = rgb[1] as i32 - c.g as i32;
            let db = rgb[2] as i32 - c.b as i32;
            dr*dr + dg*dg + db*db
          })
          .map(|(i, _)| i as u8)
          .unwrap_or(1)
      } else {
        let idx = encode_static(rgb);
        // palette index 0 is air in MagicaVoxel; shift by 1
        idx.saturating_add(1)
      };

      by_vox_chunk.entry(vc_key).or_default().push(dot_vox::Voxel {
        x: lx2, y: ly2, z: lz2, i: color_idx,
      });
    }
  }

  by_vox_chunk.into_iter().map(|(origin, voxels)| VoxChunk { origin, voxels }).collect()
}

// ---- public entry point -----------------------------------------------------

/// Write all imported chunks to a .vox file.
///
/// `chunks` is the collection of (flat_chunk_index, samples) pairs from the import callback.
/// `info` is the ImportInfo returned by `import()`.
/// If `palette_colors` is Some, those colors are used as the .vox palette (max 255 entries).
/// Otherwise a static 6-7-6 palette is used.
pub fn write_vox(
  mut writer: impl Write + Seek,
  chunks: Vec<(u64, Vec<VoxelSample>)>,
  info: &ImportInfo,
  palette_colors: Option<&[[u8; 3]]>,
) -> std::io::Result<()> {
  let palette: Vec<Color> = if let Some(colors) = palette_colors {
    // index 0 = air (black), then palette entries
    let mut pal = vec![Color { r: 0, g: 0, b: 0, a: 255 }];
    for &[r, g, b] in colors.iter().take(255) {
      pal.push(Color { r, g, b, a: 255 });
    }
    while pal.len() < 256 { pal.push(Color { r: 0, g: 0, b: 0, a: 255 }); }
    pal
  } else {
    static_palette()
  };

  let pal_for_lookup: Option<&[Color]> = if palette_colors.is_some() { Some(&palette) } else { None };

  let vox_chunks = build_vox_chunks(chunks, info, pal_for_lookup);

  // Center the scene in MagicaVoxel coordinates
  let shift = [0i32; 3]; // no shift for now; origin is whatever the voxel coords are

  let mut models = Vec::new();
  let mut nodes: Vec<SceneNode> = Vec::new();

  nodes.push(SceneNode::Transform {
    attributes: Default::default(),
    frames: vec![Frame { attributes: Default::default() }],
    child: 1,
    layer_id: 0,
  });
  nodes.push(SceneNode::Group {
    attributes: Default::default(),
    children: Vec::new(),
  });

  for vc in vox_chunks {
    let model_id = models.len() as u32;
    models.push(Model {
      size: Size { x: 256, y: 256, z: 256 },
      voxels: vc.voxels,
    });

    let transform_idx = nodes.len() as u32;
    let shape_idx = transform_idx + 1;

    let ox = vc.origin[0] * 256 + shift[0];
    let oy = vc.origin[1] * 256 + shift[1];
    let oz = vc.origin[2] * 256 + shift[2];

    nodes.push(SceneNode::Transform {
      attributes: Default::default(),
      frames: vec![Frame {
        attributes: [("_t".to_string(), format!("{ox} {oy} {oz}"))].into(),
      }],
      child: shape_idx,
      layer_id: 0,
    });
    nodes.push(SceneNode::Shape {
      attributes: Default::default(),
      models: vec![ShapeModel { model_id, attributes: Default::default() }],
    });

    let SceneNode::Group { children, .. } = &mut nodes[1] else { unreachable!() };
    children.push(transform_idx);
  }

  let data = DotVoxData {
    version: 150,
    models,
    palette,
    index_map: (0..=255).collect(),
    materials: Vec::new(),
    layers: Vec::new(),
    scenes: nodes,
  };

  data.write_vox(&mut writer)?;
  Ok(())
}
