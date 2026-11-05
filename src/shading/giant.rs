//! Siroco: gigante gaseoso con bandas deformadas, tormenta (ojo) y corrientes animadas.
use super::noise::*;
use crate::raster::{FragIn, FragmentShader, Uniforms};

pub struct GiantShader;

impl FragmentShader for GiantShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let t = u.time;
        let (x, y) = (p.x, p.y);
        let (dx, dy) = (x - 0.35, y + 0.2);
        let radius = dx.hypot(dy);
        let local = (-radius * radius / 0.11).exp();
        let angle = dy.atan2(dx);
        let wy = y + local * (angle + radius * 12.0 - t * 0.45).sin() * 0.19;
        let n1 = fbm_n(x * 4.0 + t * 0.13, wy * 8.0, p.z * 4.0, octaves(u.detail));
        let bands = (wy * 31.0 + n1 * 8.0 + (x * 5.0 + t * 0.14).cos() * 0.55).sin();
        let fine = if u.detail > 0.3 { (wy * 74.0 + n1 * 14.0).sin() * 0.12 } else { 0.0 };
        let v = bands * 0.5 + 0.5 + fine;
        let mut c = blend(rgb(91., 55., 77.), rgb(160., 95., 83.), smooth(0.12, 0.40, v));
        c = blend(c, rgb(224., 164., 111.), smooth(0.39, 0.62, v));
        c = blend(c, rgb(246., 218., 161.), smooth(0.67, 0.85, v));
        let eye = (1.0 - smooth(0.12, 0.25, radius)) * 0.62;
        let c = blend(c, rgb(149., 66., 66.), eye);
        finish(lit(c, 0.25 + 0.75 * f.normal.dot(u.light_dir).max(0.0)))
    }
}
