use crate::import::gltf::{GltfMaterial, GltfScene, GltfTexture, Triangle};
use crate::import::palette::Palette;
use crate::import::pbr::{pbr_to_voxel, PbrSample};
use crate::import::VoxelSample;
use glam::{IVec3, Vec2, Vec3, Vec4};
use std::collections::HashMap;

// ---- morton encoding --------------------------------------------------------

fn spread(mut v: u64) -> u64 {
  v = (v | (v << 16)) & 0x0000_00ff_0000_00ff;
  v = (v | (v <<  8)) & 0x00ff_00ff_00ff_00ff;
  v = (v | (v <<  4)) & 0x0f0f_0f0f_0f0f_0f0f;
  v = (v | (v <<  2)) & 0x3333_3333_3333_3333;
  v = (v | (v <<  1)) & 0x5555_5555_5555_5555;
  v
}

fn compact(mut v: u64) -> u32 {
  v &= 0x5555_5555_5555_5555;
  v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
  v = (v | (v >> 2)) & 0x0f0f_0f0f_0f0f_0f0f;
  v = (v | (v >> 4)) & 0x00ff_00ff_00ff_00ff;
  v = (v | (v >> 8)) & 0x0000_ffff_0000_ffff;
  v = (v | (v >>16)) & 0x0000_0000_ffff_ffff;
  v as u32
}

pub fn morton_encode(x: u32, y: u32, z: u32) -> u64 {
  spread(x as u64) | (spread(y as u64) << 1) | (spread(z as u64) << 2)
}

pub fn morton_decode(code: u64) -> [u32; 3] {
  [compact(code), compact(code >> 1), compact(code >> 2)]
}

// ---- color helpers ----------------------------------------------------------

fn interpolate_color(colors: [[u8; 4]; 3], bary: Vec3) -> [u8; 4] {
  let c = |i: usize| Vec4::from_array(colors[i].map(|v| v as f32));
  let out = c(0) * bary.x + c(1) * bary.y + c(2) * bary.z;
  out.as_u8vec4().to_array()
}

fn multiply_colors(a: [u8; 4], b: [u8; 4]) -> [u8; 4] {
  std::array::from_fn(|i| ((a[i] as u16 * b[i] as u16) / 255) as u8)
}

// ---- per-triangle shading data ----------------------------------------------

struct ShadeData<'a> {
  a: Vec3,
  v0: Vec3,
  v1: Vec3,
  d00: f32,
  d01: f32,
  d11: f32,
  inv_det: f32,
  vert_colors: [[u8; 4]; 3],
  base_color: [u8; 4],
  texture: Option<(&'a GltfTexture, [Vec2; 3])>,
  alpha_threshold: Option<u8>,
  roughness: f32,
  metallic: f32,
  emissive: bool,
}

impl<'a> ShadeData<'a> {
  fn new(tri: &Triangle, mat: &'a GltfMaterial) -> Self {
    let [pa, pb, pc] = tri.positions_glam();
    let v0 = pb - pa;
    let v1 = pc - pa;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let det = d00 * d11 - d01 * d01;
    let inv_det = if det.abs() < f32::EPSILON { 0.0 } else { 1.0 / det };

    let texture = mat.texture.as_ref().and_then(|tex| {
      tri.uvs().map(|uvs| (tex, uvs.map(Vec2::from_array)))
    });

    Self {
      a: pa, v0, v1, d00, d01, d11, inv_det,
      vert_colors: tri.colors(),
      base_color: mat.base_color,
      texture,
      alpha_threshold: mat.alpha_threshold,
      roughness: mat.roughness,
      metallic: mat.metallic,
      emissive: mat.emissive,
    }
  }

  fn normal(&self) -> Vec3 { self.v0.cross(self.v1) }

  fn barycentric_of(&self, p: Vec3) -> Vec3 {
    let v2 = p - self.a;
    let d20 = self.v0.dot(v2);
    let d21 = self.v1.dot(v2);
    let v = (self.d11 * d20 - self.d01 * d21) * self.inv_det;
    let w = (self.d00 * d21 - self.d01 * d20) * self.inv_det;
    Vec3::new(1.0 - v - w, v, w)
  }

  fn sample(&self, mut bary: Vec3) -> Option<[u8; 4]> {
    bary = bary.max(Vec3::ZERO);
    let sum = bary.x + bary.y + bary.z;
    if sum > f32::EPSILON { bary /= sum; }

    let vert_color = interpolate_color(self.vert_colors, bary);

    let base = match self.texture {
      Some((tex, uvs)) => {
        let mut uv = uvs[0] * bary.x + uvs[1] * bary.y + uvs[2] * bary.z;
        uv.x = tex.wrap_u.apply(uv.x);
        uv.y = tex.wrap_v.apply(uv.y);
        let (w, h) = tex.image.dimensions();
        let x = ((w - 1) as f32 * uv.x) as u32;
        let y = ((h - 1) as f32 * uv.y) as u32;
        let t = tex.image.get_pixel(x, y).0;
        multiply_colors(t, self.base_color)
      }
      None => self.base_color,
    };

    let color = multiply_colors(base, vert_color);
    if let Some(thresh) = self.alpha_threshold {
      if color[3] < thresh { return None; }
    }
    Some(color)
  }

  fn sample_at_pos(&self, pos: IVec3) -> Option<[u8; 4]> {
    let bary = self.barycentric_of(pos.as_vec3());
    self.sample(bary)
  }
}

// ---- voxel emit -------------------------------------------------------------

struct Store<'a> {
  palette: Option<&'a Palette>,
  /// local_pos (within chunk) -> raw color [u8; 4] and material props
  cells: HashMap<[i32; 3], ([u8; 4], f32, f32, bool)>,
}

impl<'a> Store<'a> {
  fn new(palette: Option<&'a Palette>) -> Self {
    Self { palette, cells: HashMap::new() }
  }

  fn add(&mut self, local: [i32; 3], raw_color: [u8; 4], roughness: f32, metallic: f32, emissive: bool) {
    self.cells.entry(local).or_insert((raw_color, roughness, metallic, emissive));
  }

  fn into_samples(self) -> Vec<VoxelSample> {
    let mut samples: Vec<VoxelSample> = self.cells.into_iter().map(|(pos, (color, roughness, metallic, emissive))| {
      let rgb: [u8; 3] = if let Some(pal) = self.palette {
        pal.nearest([color[0], color[1], color[2]])
      } else {
        [color[0], color[1], color[2]]
      };
      let voxel = pbr_to_voxel(&PbrSample { rgb, roughness, metallic, emissive });
      let morton = morton_encode(pos[0] as u32, pos[1] as u32, pos[2] as u32);
      VoxelSample { morton, voxel }
    }).collect();
    samples.sort_unstable_by_key(|s| s.morton);
    samples
  }
}

// ---- triangle rasterization -------------------------------------------------

const EPSILON: f32 = -0.001;

fn voxelize_line(
  store: &mut Store,
  shade: &ShadeData,
  p1: Vec3, p2: Vec3,
  range_min: IVec3, range_max: IVec3,
  chunk_origin: IVec3,
) {
  let ray_dir = p2 - p1;
  if !ray_dir.is_finite() || ray_dir.length_squared() < f32::EPSILON { return; }
  let len = ray_dir.length();
  let ray_dir = ray_dir / len;
  let inv_dir = Vec3::ONE / ray_dir;

  let box_min = range_min.as_vec3();
  let box_max = range_max.as_vec3();

  let mut t_entry = 0.0_f32;
  let mut t_exit = len;

  for i in 0..3 {
    if ray_dir[i].abs() < f32::EPSILON {
      if p1[i] < box_min[i] || p1[i] > box_max[i] { return; }
      continue;
    }
    let t0 = (box_min[i] - p1[i]) * inv_dir[i];
    let t1 = (box_max[i] - p1[i]) * inv_dir[i];
    let (tn, tf) = if inv_dir[i] < 0.0 { (t1, t0) } else { (t0, t1) };
    t_entry = t_entry.max(tn);
    t_exit  = t_exit.min(tf);
  }
  if t_entry > t_exit { return; }

  let start_pos = if t_entry > 0.0 { p1 + ray_dir * t_entry } else { p1 };
  let end = p2.floor().as_ivec3();
  let mut voxel_pos = start_pos.floor().as_ivec3();

  let step = ray_dir.signum().as_ivec3();
  let step_pos = step.max(IVec3::ZERO);
  let next = (voxel_pos + step_pos).as_vec3();
  let t_delta = inv_dir.abs();
  let mut t_max = (next - p1) * inv_dir;

  let max_steps = ((t_exit - t_entry) as u32 + 2) * 3;

  for _ in 0..max_steps {
    if let Some(color) = shade.sample_at_pos(voxel_pos) {
      let local = (voxel_pos - chunk_origin).to_array();
      store.add(local, color, shade.roughness, shade.metallic, shade.emissive);
    }
    if voxel_pos == end { break; }
    let axis = t_max.min_position();
    if t_max[axis] > t_exit + 0.01 { break; }
    t_max[axis] += t_delta[axis];
    voxel_pos[axis] += step[axis];
  }
}

fn voxelize_triangle<const FAT: bool>(
  store: &mut Store,
  shade: &ShadeData,
  tri: &Triangle,
  range_min: IVec3, range_max: IVec3,
  chunk_origin: IVec3,
) {
  let [pa, pb, pc] = tri.positions_glam();

  // Conservative wireframe rasterization first
  voxelize_line(store, shade, pa, pb, range_min, range_max, chunk_origin);
  voxelize_line(store, shade, pb, pc, range_min, range_max, chunk_origin);
  voxelize_line(store, shade, pa, pc, range_min, range_max, chunk_origin);

  let normal = shade.normal();
  let d_axis = normal.abs().max_position();
  let u_axis = (d_axis + 1) % 3;
  let v_axis = (d_axis + 2) % 3;

  let nu = normal[u_axis];
  let nv = normal[v_axis];
  let nd_inv = 1.0 / normal[d_axis];
  let plane_d = normal.dot(pa);

  let a2 = Vec2::new(pa[u_axis], pa[v_axis]);
  let b2 = Vec2::new(pb[u_axis], pb[v_axis]);
  let c2 = Vec2::new(pc[u_axis], pc[v_axis]);
  let ab = b2 - a2;
  let ac = c2 - a2;
  let area = ab.perp_dot(ac);
  let area_inv = 1.0 / area;
  if area.abs() < f32::EPSILON { return; }

  let delta_d = if FAT {
    0.5 * ((nu * nd_inv).abs() + (nv * nd_inv).abs())
  } else { 0.0 };

  let umin = a2.min(b2).min(c2).floor().as_ivec2();
  let umax = a2.max(b2).max(c2).ceil().as_ivec2();

  let u_start = umin.x.max(range_min[u_axis]);
  let u_end   = umax.x.min(range_max[u_axis]);
  let v_start = umin.y.max(range_min[v_axis]);
  let v_end   = umax.y.min(range_max[v_axis]);

  for u in u_start..=u_end {
    for v in v_start..=v_end {
      let p = Vec2::new(u as f32 + 0.5, v as f32 + 0.5);
      let ap = p - a2;
      let c_bary = ab.perp_dot(ap) * area_inv;
      let b_bary = ap.perp_dot(ac) * area_inv;
      let a_bary = 1.0 - c_bary - b_bary;

      if a_bary >= EPSILON && b_bary >= EPSILON && c_bary >= EPSILON {
        let depth = (plane_d - nu * p.x - nv * p.y) * nd_inv;
        let bary = Vec3::new(a_bary, b_bary, c_bary);
        let Some(color) = shade.sample(bary) else { continue };

        if FAT {
          let d_min = (depth - delta_d).floor() as i32;
          let d_max = (depth + delta_d).floor() as i32;
          for d in d_min..=d_max {
            let mut vp = [0i32; 3];
            vp[u_axis] = u; vp[v_axis] = v; vp[d_axis] = d;
            let vp = IVec3::from_array(vp);
            if vp.cmpge(range_min).all() && vp.cmplt(range_max).all() {
              let local = (vp - chunk_origin).to_array();
              store.add(local, color, shade.roughness, shade.metallic, shade.emissive);
            }
          }
        } else {
          let d = depth.floor() as i32;
          let mut vp = [0i32; 3];
          vp[u_axis] = u; vp[v_axis] = v; vp[d_axis] = d;
          let vp = IVec3::from_array(vp);
          if vp.cmpge(range_min).all() && vp.cmplt(range_max).all() {
            let local = (vp - chunk_origin).to_array();
            store.add(local, color, shade.roughness, shade.metallic, shade.emissive);
          }
        }
      }
    }
  }
}

// ---- public entry point -----------------------------------------------------

/// Rasterize triangles for one chunk into morton-sorted VoxelSamples.
///
/// `chunk_voxel_origin` is the chunk's position in global voxel space.
/// World positions are converted to voxel space with `(p - world_min) / voxel_size`.
pub fn voxelize_chunk(
  scene: &GltfScene,
  triangle_indices: &[usize],
  chunk_voxel_origin: [u32; 3],
  world_min: [f32; 3],
  voxel_size: f32,
  chunk_voxels: u32,
  palette: Option<&Palette>,
) -> Vec<VoxelSample> {
  let origin = IVec3::from_array(chunk_voxel_origin.map(|v| v as i32));
  let range_min = origin;
  let range_max = origin + IVec3::splat(chunk_voxels as i32);

  let mut store = Store::new(palette);

  for &tri_idx in triangle_indices {
    let tri_world = &scene.triangles[tri_idx];
    let mat = &scene.materials[tri_world.material_idx];

    // Transform triangle vertices from world space to voxel space
    let mut tri_voxel = *tri_world;
    let wm = Vec3::from_array(world_min);
    for v in &mut tri_voxel.vertices {
      let p = (Vec3::from_array(v.pos) - wm) / voxel_size;
      v.pos = p.to_array();
    }

    let shade = ShadeData::new(&tri_voxel, mat);

    voxelize_triangle::<true>(&mut store, &shade, &tri_voxel, range_min, range_max, origin);
  }

  store.into_samples()
}
