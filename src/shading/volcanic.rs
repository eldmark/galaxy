//! Fornax: placas de basalto (celdas 3D) con magma animado, bump mapping y parallax.
//! La iluminación se calcula en espacio local (luz/vista/normal transformadas con el modelo).
use super::noise::*;
use crate::math::Vec3;
use crate::raster::{FragIn, FragmentShader, Uniforms};

pub struct VolcanicShader {
    pub bump: bool,
    pub parallax: bool,
}
impl VolcanicShader {
    pub fn new() -> Self {
        Self { bump: true, parallax: true }
    }
}
impl Default for VolcanicShader {
    fn default() -> Self {
        Self::new()
    }
}

struct Cell {
    border: f32,
    grad: Vec3,
}
fn cell_surface(p: Vec3) -> Cell {
    let base = [p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32];
    let (mut nearest, mut second) = (f32::INFINITY, f32::INFINITY);
    let (mut c1, mut c2) = (Vec3::ZERO, Vec3::ZERO);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y, z) = (base[0] + dx, base[1] + dy, base[2] + dz);
                let c = Vec3::new(
                    x as f32 + 0.15 + hash(x, y, z) * 0.7,
                    y as f32 + 0.15 + hash(x + 31, y, z) * 0.7,
                    z as f32 + 0.15 + hash(x, y + 37, z) * 0.7,
                );
                let d = (p - c).length_sq();
                if d < nearest {
                    second = nearest;
                    c2 = c1;
                    nearest = d;
                    c1 = c;
                } else if d < second {
                    second = d;
                    c2 = c;
                }
            }
        }
    }
    let d1 = nearest.sqrt().max(1e-6);
    let d2 = second.sqrt().max(1e-6);
    Cell { border: (d2 - d1).max(0.0), grad: (p - c2) * (1.0 / d2) - (p - c1) * (1.0 / d1) }
}

struct Plate {
    flow: f32,
    rock: f32,
    height: f32,
    crack: f32,
    slope: Vec3,
}
fn plate(p: Vec3, t: f32, o: u32) -> Plate {
    let flow = fbm_n(p.x * 3.5 + t * 0.12, p.y * 3.5, p.z * 3.5, o);
    let w = Vec3::new(p.x * 7.0 + flow * 0.55, p.y * 7.0, p.z * 7.0 - flow * 0.35);
    let cell = cell_surface(w);
    let plateau = smooth(0.03, 0.13, cell.border);
    let rock = fbm_n(p.x * 13.0, p.y * 13.0, p.z * 13.0, o);
    let tt = (cell.border - 0.03) / 0.10;
    let k = if (0.0..1.0).contains(&tt) { 0.55 * 6.0 * tt * (1.0 - tt) / 0.10 * 7.0 } else { 0.0 };
    Plate { flow, rock, crack: 1.0 - plateau, height: 0.25 + 0.55 * plateau + 0.20 * rock, slope: cell.grad * k }
}

fn parallax_pos(p: Vec3, n: Vec3, view: Vec3, t: f32, o: u32) -> Vec3 {
    let h = plate(p, t, o).height;
    let facing = n.dot(view).max(0.35);
    let tangent = view - n * n.dot(view);
    let dist = |h: f32| ((1.0 - h) * 0.03 / facing).min(0.06);
    let first = p - tangent * dist(h);
    let h2 = plate(first, t, o).height;
    p - tangent * dist((h + h2) * 0.5)
}

impl FragmentShader for VolcanicShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let o = octaves(u.detail);
        let lite = u.detail >= 0.45; // por debajo: sin parallax ni bump
        let n = to_local(&u.model, f.normal);
        let light = light_local(u);
        let view = to_local(&u.model, f.view_dir);
        let sample = if self.parallax && lite { parallax_pos(p, n, view, u.time, o) } else { p };
        let s = plate(sample, u.time, o);
        let mut c = blend(rgb(27., 26., 39.), rgb(86., 55., 62.), smooth(0.28, 0.72, s.rock));
        c = blend(c, rgb(159., 49., 47.), s.crack * 0.67);
        c = blend(c, rgb(255., 113., 38.), s.crack * smooth(0.38, 0.61, s.flow));
        c = blend(c, rgb(255., 219., 111.), s.crack * smooth(0.62, 0.78, s.flow) * 0.65);
        let ln = if self.bump && lite {
            let eps = 0.012;
            let mut rs = [0.0f32; 3];
            let base = [sample.x, sample.y, sample.z];
            for a in 0..3 {
                let mut q = base;
                q[a] += eps;
                rs[a] = 0.20 * (fbm_n(q[0] * 13.0, q[1] * 13.0, q[2] * 13.0, o) - s.rock) / eps;
            }
            let slope = s.slope * 0.014 + Vec3::new(rs[0], rs[1], rs[2]) * 0.10;
            let tang = slope - n * slope.dot(n);
            (n - tang).normalized()
        } else {
            n
        };
        let diffuse = 0.20 + 0.8 * ln.dot(light).max(0.0);
        let half = (light + view).normalized();
        let spec = ln.dot(half).max(0.0).powi(24) * (1.0 - s.crack) * (36.0 / 255.0);
        let lit_c = add_all(lit(c, diffuse), spec);
        // Las grietas emiten luz incluso en el lado oscuro.
        finish(blend(lit_c, c, s.crack * 0.7))
    }
}
