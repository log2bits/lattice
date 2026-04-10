use glam::{Mat4, Vec3};
use image::RgbaImage;
use std::path::Path;
use std::sync::Arc;

// ---- types ------------------------------------------------------------------

#[derive(Clone, Copy)]
pub enum WrapMode { ClampToEdge, MirroredRepeat, Repeat }

impl WrapMode {
  pub fn apply(self, c: f32) -> f32 {
    match self {
      Self::ClampToEdge => c.clamp(0.0, 1.0),
      Self::Repeat => c.rem_euclid(1.0),
      Self::MirroredRepeat => {
        let m = c.rem_euclid(2.0);
        if m > 1.0 { 2.0 - m } else { m }
      }
    }
  }
}

pub struct GltfTexture {
  pub image: Arc<RgbaImage>,
  pub wrap_u: WrapMode,
  pub wrap_v: WrapMode,
}

pub struct GltfMaterial {
  /// sRGB base color (or emissive color if emissive).
  pub base_color: [u8; 4],
  pub roughness: f32,
  pub metallic: f32,
  pub emissive: bool,
  pub alpha_threshold: Option<u8>,
  pub texture: Option<GltfTexture>,
}

#[derive(Clone, Copy)]
pub struct Vertex {
  pub pos: [f32; 3],
  /// [255,255,255,255] when no vertex color attribute is present.
  pub color: [u8; 4],
  /// [NAN, NAN] when no UV attribute is present.
  pub uv: [f32; 2],
}

impl Vertex {
  pub fn uv(&self) -> Option<[f32; 2]> {
    (!self.uv[0].is_nan()).then_some(self.uv)
  }
}

#[derive(Clone, Copy)]
pub struct Triangle {
  pub vertices: [Vertex; 3],
  pub material_idx: usize,
}

impl Triangle {
  pub fn uvs(&self) -> Option<[[f32; 2]; 3]> {
    let [a, b, c] = &self.vertices;
    Some([a.uv()?, b.uv()?, c.uv()?])
  }

  pub fn colors(&self) -> [[u8; 4]; 3] {
    self.vertices.map(|v| v.color)
  }

  pub fn positions_glam(&self) -> [Vec3; 3] {
    self.vertices.map(|v| Vec3::from_array(v.pos))
  }
}

pub struct GltfScene {
  pub triangles: Vec<Triangle>,
  pub materials: Vec<GltfMaterial>,
  pub bounds_min: [f32; 3],
  pub bounds_max: [f32; 3],
}

// ---- image conversion -------------------------------------------------------

fn convert_image(data: gltf::image::Data) -> anyhow::Result<Arc<RgbaImage>> {
  use bytemuck::Pod;
  use gltf::image::Format;
  use image::buffer::ConvertBuffer;
  use image::{ImageBuffer, Luma, LumaA, Rgb, Rgba};

  fn convert<P: image::Pixel>(data: gltf::image::Data) -> anyhow::Result<Arc<RgbaImage>>
  where
    P::Subpixel: Pod,
    ImageBuffer<P, Vec<P::Subpixel>>: ConvertBuffer<RgbaImage>,
  {
    let pixels: Vec<P::Subpixel> = match bytemuck::try_cast_vec::<u8, P::Subpixel>(data.pixels) {
      Ok(v) => v,
      Err((_, v)) => bytemuck::pod_collect_to_vec(&v),
    };
    ImageBuffer::from_vec(data.width, data.height, pixels)
      .map(|img: ImageBuffer<P, _>| Arc::new(img.convert()))
      .ok_or_else(|| anyhow::anyhow!("invalid image dimensions"))
  }

  match data.format {
    Format::R8G8B8A8 => RgbaImage::from_vec(data.width, data.height, data.pixels)
      .map(Arc::new)
      .ok_or_else(|| anyhow::anyhow!("invalid image dimensions")),
    Format::R8G8B8   => convert::<Rgb<u8>>(data),
    Format::R8G8     => convert::<LumaA<u8>>(data),
    Format::R8       => convert::<Luma<u8>>(data),
    Format::R16G16B16A16 => convert::<Rgba<u16>>(data),
    Format::R16G16B16    => convert::<Rgb<u16>>(data),
    Format::R16G16       => convert::<LumaA<u16>>(data),
    Format::R16          => convert::<Luma<u16>>(data),
    Format::R32G32B32FLOAT    => convert::<Rgb<f32>>(data),
    Format::R32G32B32A32FLOAT => convert::<Rgba<f32>>(data),
  }
}

// ---- material parsing -------------------------------------------------------

fn wrap_mode(m: gltf::texture::WrappingMode) -> WrapMode {
  match m {
    gltf::texture::WrappingMode::ClampToEdge    => WrapMode::ClampToEdge,
    gltf::texture::WrappingMode::MirroredRepeat => WrapMode::MirroredRepeat,
    gltf::texture::WrappingMode::Repeat         => WrapMode::Repeat,
  }
}

fn parse_texture(
  mat: &gltf::Material,
  images: &[Arc<RgbaImage>],
) -> anyhow::Result<(Option<GltfTexture>, u32)> {
  // try emissive texture, then albedo, then specular-glossiness diffuse
  let pick = mat.emissive_texture()
    .map(|i| i)
    .or_else(|| mat.pbr_metallic_roughness().base_color_texture().map(|i| i))
    .or_else(|| {
      mat.pbr_specular_glossiness()
        .and_then(|s| s.diffuse_texture())
    });

  let Some(info) = pick else { return Ok((None, 0)) };

  let tex_coord = info.tex_coord();
  let img_idx = info.texture().source().index();
  let image = images.get(img_idx)
    .ok_or_else(|| anyhow::anyhow!("texture image index out of bounds"))?;

  let tex = GltfTexture {
    image: Arc::clone(image),
    wrap_u: wrap_mode(info.texture().sampler().wrap_s()),
    wrap_v: wrap_mode(info.texture().sampler().wrap_t()),
  };

  Ok((Some(tex), tex_coord))
}

fn parse_material(
  mat: &gltf::Material,
  images: &[Arc<RgbaImage>],
) -> anyhow::Result<(GltfMaterial, u32)> {
  let alpha_threshold = match mat.alpha_mode() {
    gltf::material::AlphaMode::Opaque => None,
    gltf::material::AlphaMode::Mask   => {
      let cutoff = mat.alpha_cutoff().unwrap_or(0.5);
      Some((cutoff * 255.0) as u8)
    }
    gltf::material::AlphaMode::Blend => Some(250),
  };

  let emissive = mat.emissive_factor().iter().any(|&c| c > 0.0);

  let base_color: [u8; 4] = if emissive {
    let [r, g, b] = mat.emissive_factor().map(|c| (c * 255.0) as u8);
    [r, g, b, 255]
  } else {
    mat.pbr_metallic_roughness()
      .base_color_factor()
      .map(|c| (c * 255.0) as u8)
  };

  let pbr = mat.pbr_metallic_roughness();
  let roughness = pbr.roughness_factor();
  let metallic  = pbr.metallic_factor();

  let (texture, tex_coord) = parse_texture(mat, images)?;

  Ok((GltfMaterial { base_color, roughness, metallic, emissive, alpha_threshold, texture }, tex_coord))
}

// ---- mesh parsing -----------------------------------------------------------

struct MeshScratch {
  positions: Vec<[f32; 3]>,
  uvs: Vec<[f32; 2]>,
  colors: Vec<[u8; 4]>,
  indices: Vec<u32>,
}

impl Default for MeshScratch {
  fn default() -> Self {
    Self {
      positions: Vec::new(),
      uvs: Vec::new(),
      colors: Vec::new(),
      indices: Vec::new(),
    }
  }
}

fn push_triangle(
  [i0, i1, i2]: [u32; 3],
  scratch: &MeshScratch,
  material_idx: usize,
  tex_coord: u32,
  out: &mut Vec<Triangle>,
) {
  let [i0, i1, i2] = [i0 as usize, i1 as usize, i2 as usize];

  if i0 >= scratch.positions.len()
    || i1 >= scratch.positions.len()
    || i2 >= scratch.positions.len()
  {
    return;
  }

  // Use texture UVs only when the mesh actually has them and they match tex_coord 0.
  // (tex_coord > 0 paths would need a second UV set; not supported, fall back to None.)
  let has_uv = !scratch.uvs.is_empty() && tex_coord == 0;

  let vertex = |i: usize| Vertex {
    pos: scratch.positions[i],
    color: scratch.colors.get(i).copied().unwrap_or([255, 255, 255, 255]),
    uv: if has_uv { scratch.uvs.get(i).copied().unwrap_or([f32::NAN; 2]) }
        else { [f32::NAN; 2] },
  };

  out.push(Triangle {
    vertices: [vertex(i0), vertex(i1), vertex(i2)],
    material_idx,
  });
}

fn parse_primitive(
  prim: gltf::Primitive,
  transform: Mat4,
  bounds_min: &mut [f32; 3],
  bounds_max: &mut [f32; 3],
  mat_idx: usize,
  tex_coord: u32,
  buffers: &[gltf::buffer::Data],
  scratch: &mut MeshScratch,
  out: &mut Vec<Triangle>,
) {
  let reader = prim.reader(|buf| Some(&buffers[buf.index()]));

  let Some(pos_iter) = reader.read_positions() else { return };

  scratch.positions.clear();
  for p in pos_iter {
    let wp = transform.transform_point3(Vec3::from(p)).to_array();
    for i in 0..3 {
      bounds_min[i] = bounds_min[i].min(wp[i]);
      bounds_max[i] = bounds_max[i].max(wp[i]);
    }
    scratch.positions.push(wp);
  }

  scratch.uvs.clear();
  if let Some(iter) = reader.read_tex_coords(0) {
    scratch.uvs.extend(iter.into_f32());
  }

  scratch.colors.clear();
  if let Some(iter) = reader.read_colors(0) {
    scratch.colors.extend(iter.into_rgba_u8());
  }

  scratch.indices.clear();
  if let Some(iter) = reader.read_indices() {
    scratch.indices.extend(iter.into_u32());
  } else {
    scratch.indices.extend(0..scratch.positions.len() as u32);
  }

  match prim.mode() {
    gltf::mesh::Mode::Triangles => {
      let (chunks, _) = scratch.indices.as_chunks::<3>();
      for &tri in chunks {
        push_triangle(tri, scratch, mat_idx, tex_coord, out);
      }
    }
    gltf::mesh::Mode::TriangleStrip => {
      for (i, win) in scratch.indices.windows(3).enumerate() {
        let [i0, i1, i2]: [u32; 3] = win.try_into().unwrap();
        if i % 2 == 0 { push_triangle([i0, i1, i2], scratch, mat_idx, tex_coord, out); }
        else          { push_triangle([i0, i2, i1], scratch, mat_idx, tex_coord, out); }
      }
    }
    gltf::mesh::Mode::TriangleFan => {
      if scratch.indices.len() >= 3 {
        let i0 = scratch.indices[0];
        for win in scratch.indices[1..].windows(2) {
          let [i1, i2]: [u32; 2] = win.try_into().unwrap();
          push_triangle([i0, i1, i2], scratch, mat_idx, tex_coord, out);
        }
      }
    }
    _ => {}
  }
}

// ---- scene traversal --------------------------------------------------------

struct MeshInstance<'a> {
  mesh: gltf::Mesh<'a>,
  transform: Mat4,
}

fn collect_instances<'a>(
  node: &gltf::Node<'a>,
  parent: Mat4,
  out: &mut Vec<MeshInstance<'a>>,
) {
  let local = Mat4::from_cols_array_2d(&node.transform().matrix());
  let global = parent * local;
  if let Some(mesh) = node.mesh() {
    out.push(MeshInstance { mesh, transform: global });
  }
  for child in node.children() {
    collect_instances(&child, global, out);
  }
}

// ---- public API -------------------------------------------------------------

/// Load a glTF/glb file and return the full scene ready for voxelization.
pub fn load(path: &Path) -> anyhow::Result<GltfScene> {
  use rayon::prelude::*;

  let base = path.parent();

  let mut gltf_file = {
    let f = std::fs::File::open(path)?;
    gltf::Gltf::from_reader(std::io::BufReader::new(f))?
  };

  let buffers = gltf::import_buffers(&gltf_file.document, base, gltf_file.blob.take())?;

  let images: Vec<Arc<RgbaImage>> = gltf_file
    .images()
    .collect::<Vec<_>>()
    .into_par_iter()
    .map(|img| {
      let data = gltf::image::Data::from_source(img.source(), base, &buffers)?;
      convert_image(data)
    })
    .collect::<anyhow::Result<Vec<_>>>()?;

  // Materials
  let mat_results: Vec<(GltfMaterial, u32)> = gltf_file
    .document
    .materials()
    .map(|m| parse_material(&m, &images))
    .collect::<anyhow::Result<Vec<_>>>()?;

  let (mut materials, tex_coords): (Vec<GltfMaterial>, Vec<u32>) =
    mat_results.into_iter().unzip();

  // Default fallback material
  materials.push(GltfMaterial {
    base_color: [255, 255, 255, 255],
    roughness: 1.0,
    metallic: 0.0,
    emissive: false,
    alpha_threshold: None,
    texture: None,
  });
  let fallback_tex_coord = 0u32;

  // Gather instances
  let mut instances = Vec::new();
  for scene in gltf_file.document.scenes() {
    for node in scene.nodes() {
      collect_instances(&node, Mat4::IDENTITY, &mut instances);
    }
  }

  let mut triangles = Vec::new();
  let mut bounds_min = [f32::MAX; 3];
  let mut bounds_max = [f32::MIN; 3];
  let mut scratch = MeshScratch::default();

  for inst in instances {
    for prim in inst.mesh.primitives() {
      let mat_idx = prim.material().index().unwrap_or(materials.len() - 1);
      let tex_coord = tex_coords.get(mat_idx).copied().unwrap_or(fallback_tex_coord);
      parse_primitive(
        prim, inst.transform,
        &mut bounds_min, &mut bounds_max,
        mat_idx, tex_coord,
        &buffers, &mut scratch, &mut triangles,
      );
    }
  }

  // If scene had no geometry, avoid degenerate bounds
  if bounds_min[0] > bounds_max[0] {
    bounds_min = [0.0; 3];
    bounds_max = [1.0; 3];
  }

  Ok(GltfScene { triangles, materials, bounds_min, bounds_max })
}

/// Return the world-space bounding box of all mesh geometry.
pub fn scene_bounds(path: &Path) -> anyhow::Result<([f32; 3], [f32; 3])> {
  let scene = load(path)?;
  Ok((scene.bounds_min, scene.bounds_max))
}
