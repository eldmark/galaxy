//! Verdalia: continentes + océano, casquetes polares, ciudades nocturnas, nubes animadas con sombra.
use super::life::LifeTimeline;
use super::noise::*;
use super::normalmap::{self, Surface, Tiling};
use crate::raster::{FragIn, FragmentShader, Uniforms};
use std::f32::consts::PI;

pub struct RockyShader {
    life: LifeTimeline,
}
impl RockyShader {
    pub fn new() -> Self {
        Self { life: LifeTimeline::new(48, 24, 11, false) }
    }
}
impl Default for RockyShader {
    fn default() -> Self {
        Self::new()
    }
}

fn clouds(x: f32, y: f32, z: f32, t: f32, o: u32) -> f32 {
    fbm_n(x * 8.0 + t * 0.24, y * 8.0 + t * 0.07, z * 8.0 - t * 0.17, o)
}

impl FragmentShader for RockyShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let o = octaves(u.detail);
        let lo = light_local(u);
        let uu = p.x.atan2(p.z) / (2.0 * PI) + 0.5;
        let vv = p.y.clamp(-1.0, 1.0).asin() / PI + 0.5;
        let continent = fbm_n(p.x * 3.5 + 4.0, p.y * 3.5, p.z * 3.5, o);
        let cells = self.life.sample(uu, vv, u.time);
        let region = continent + if cells { 0.09 } else { -0.04 };
        let land = smooth(0.44, 0.54, region);
        let polar = smooth(0.70, 0.88, p.y.abs());
        let det = fbm_n(p.x * 15.0, p.y * 15.0, p.z * 15.0, o);
        let mut c = blend(rgb(7., 35., 68.), rgb(24., 115., 137.), smooth(0.24, 0.68, det));
        c = blend(c, rgb(79., 171., 153.), (1.0 - smooth(0.46, 0.59, region)) * land * 0.7);
        let mut ground = blend(rgb(44., 96., 78.), rgb(178., 144., 87.), smooth(0.36, 0.72, det + p.y.abs() * 0.1));
        ground = blend(ground, rgb(99., 71., 59.), smooth(0.63, 0.8, det));
        c = blend(c, ground, land);
        c = blend(c, rgb(210., 227., 221.), polar * (0.65 + det * 0.35));
        let nl = f.normal.dot(u.light_dir);
        // Mapa de normales de grava solo en tierra firme (menos en los casquetes de hielo liso).
        let ground_n = if land > 0.05 {
            let pn = normalmap::perturb_world(normalmap::map(Surface::Gravel), f, u, Tiling { tile_m: 6.0, strength: 1.0 });
            (f.normal + (pn - f.normal) * (land * (1.0 - polar * 0.6))).normalized()
        } else {
            f.normal
        };
        let d = ground_n.dot(u.light_dir).max(0.0);
        // Brillo especular del océano: reflejo de la luz hacia la cámara.
        let r = f.normal * (2.0 * nl) - u.light_dir;
        let spec = if nl > 0.0 { r.dot(f.view_dir).max(0.0).powi(20) * (1.0 - land) * (105.0 / 255.0) } else { 0.0 };
        c = lit(c, 0.23 + 0.77 * d);
        c = add_all(c, spec);
        if land > 0.05 && nl > 0.0 {
            // brillo mate de la roca mojada/pulida, solo visible con relieve fino
            let hv = (u.light_dir + f.view_dir).normalized();
            c = add_all(c, ground_n.dot(hv).max(0.0).powi(14) * 0.05 * land);
        }
        let cloud = smooth(0.54, 0.63, clouds(p.x, p.y, p.z, u.time, o));
        let sh = smooth(0.54, 0.63, clouds(p.x - lo.x * 0.045, p.y - lo.y * 0.045, p.z - lo.z * 0.045, u.time, o));
        c = lit(c, 1.0 - sh * 0.35 * d);
        if u.detail > 0.4 {
            let cities = noise(p.x * 75.0 + 12.0, p.y * 75.0, p.z * 75.0);
            let settle = smooth(0.71, 0.82, cities) * land * (1.0 - polar);
            c = blend(c, rgb(255., 190., 79.), settle * (1.0 - smooth(0.05, 0.30, d)) * 0.92);
        }
        finish(blend(c, rgb(232., 244., 241.), cloud * 0.83 * (0.4 + 0.6 * d)))
    }
}
