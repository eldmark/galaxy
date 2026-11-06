//! Mutante: cara pixelada 16x16 que muta de vaca a cerdo; el azar lo guía HighLife (precalculado).
use super::life::LifeTimeline;
use super::noise::*;
use super::normalmap::{self, Surface, Tiling};
use crate::raster::{FragIn, FragmentShader, Uniforms};

pub struct PixelShader {
    life: LifeTimeline,
}
impl PixelShader {
    pub fn new() -> Self {
        Self { life: LifeTimeline::new(32, 32, 27, true) }
    }
}
impl Default for PixelShader {
    fn default() -> Self {
        Self::new()
    }
}

fn rect(x: i32, y: i32, a: i32, b: i32, c: i32, d: i32) -> bool {
    x >= a && x < c && y >= b && y < d
}
fn face(x: i32, y: i32, pig: bool) -> Rgb {
    if pig {
        let mut c = rgb(238., 152., 158.);
        if rect(x, y, 0, 0, 16, 4) || rect(x, y, 0, 12, 16, 16) { c = rgb(244., 174., 180.); }
        if rect(x, y, 3, 5, 5, 8) || rect(x, y, 11, 5, 13, 8) { c = rgb(250., 246., 238.); }
        if rect(x, y, 4, 6, 5, 8) || rect(x, y, 11, 6, 12, 8) { c = rgb(43., 39., 47.); }
        if rect(x, y, 3, 9, 13, 14) { c = rgb(199., 112., 124.); }
        if rect(x, y, 4, 9, 12, 13) { c = rgb(255., 196., 201.); }
        if rect(x, y, 5, 10, 7, 12) || rect(x, y, 9, 10, 11, 12) { c = rgb(126., 67., 72.); }
        c
    } else {
        let mut c = rgb(110., 77., 57.);
        if rect(x, y, 0, 0, 16, 3) || rect(x, y, 0, 12, 16, 16) { c = rgb(84., 57., 44.); }
        if rect(x, y, 3, 4, 6, 8) || rect(x, y, 10, 4, 13, 8) { c = rgb(224., 218., 200.); }
        if rect(x, y, 4, 6, 5, 8) || rect(x, y, 11, 6, 12, 8) { c = rgb(36., 32., 34.); }
        if rect(x, y, 4, 9, 12, 14) { c = rgb(190., 177., 159.); }
        if rect(x, y, 5, 10, 7, 12) || rect(x, y, 9, 10, 11, 12) { c = rgb(50., 46., 45.); }
        if rect(x, y, 6, 3, 10, 6) { c = rgb(218., 210., 194.); }
        c
    }
}

impl FragmentShader for PixelShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let phase = u.time.max(0.0).rem_euclid(24.0) / 12.0;
        let progress = if phase <= 1.0 { phase } else { 2.0 - phase };
        let gx = (p.x + 0.75) / 1.5 * 16.0;
        let gy = (0.75 - p.y) / 1.5 * 16.0;
        let x = (gx / 16.0).clamp(0.0, 0.999_999) * 16.0;
        let y = (gy / 16.0).clamp(0.0, 0.999_999) * 16.0;
        let (xi, yi) = (x.floor() as i32, y.floor() as i32);
        let cell = self.life.sample(xi as f32 / 16.0, yi as f32 / 16.0, u.time);
        let thr = hash(xi, yi, 91) * 0.75 + if cell { 0.08 } else { 0.23 };
        let mut c = face(xi, yi, progress > thr);
        if gx.rem_euclid(1.0).min(gy.rem_euclid(1.0)) < 0.035 {
            c = lit(c, 0.85);
        }
        let n = normalmap::perturb_world(normalmap::map(Surface::Steps), f, u, Tiling { tile_m: 4.0, strength: 1.0 });
        finish(lit(c, 0.37 + 0.63 * n.dot(u.light_dir).max(0.0)))
    }
}
