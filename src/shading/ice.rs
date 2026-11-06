//! Nix: hielo con vetas cristalinas (ondas deformadas por ruido) y auroras tenues.
use super::noise::*;
use super::normalmap::{self, Surface, Tiling};
use crate::raster::{FragIn, FragmentShader, Uniforms};

pub struct IceShader;

impl FragmentShader for IceShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let o = octaves(u.detail);
        let frost = fbm_n(p.x * 5.0 + 8.0, p.y * 5.0, p.z * 5.0, o);
        let cry = (p.x * 24.0 + frost * 11.0).sin() * (p.y * 21.0 - frost * 8.0).cos();
        let veins = 1.0 - smooth(0.03, 0.18, cry.abs());
        let haze = fbm_n(p.x * 12.0, p.y * 12.0 + u.time * 0.06, p.z * 12.0, o);
        let mut c = blend(rgb(32., 38., 102.), rgb(69., 129., 179.), smooth(0.29, 0.6, frost));
        c = blend(c, rgb(172., 218., 231.), smooth(0.48, 0.7, haze));
        c = blend(c, rgb(229., 242., 249.), veins * 0.8);
        let aurora = smooth(0.45, 0.68, frost) * (p.y * 12.0 + p.x * 6.0).sin().abs();
        c = blend(c, rgb(132., 229., 207.), aurora * 0.25);
        let n = normalmap::perturb_world(normalmap::map(Surface::Ice), f, u, Tiling { tile_m: 5.0, strength: 1.0 });
        let d = n.dot(u.light_dir).max(0.0);
        // Brillo de escarcha (especular suave) en las vetas.
        let h = (u.light_dir + f.view_dir).normalized();
        let spec = n.dot(h).max(0.0).powi(30) * 0.18 * veins;
        finish(add_all(lit(c, 0.22 + 0.78 * d), spec))
    }
}
