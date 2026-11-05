//! Sol: emisivo, granulación animada, manchas y oscurecimiento al limbo. No usa la luz.
use super::noise::*;
use crate::raster::{FragIn, FragmentShader, Uniforms};

pub struct SunShader;

impl FragmentShader for SunShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let t = u.time;
        let o = octaves(u.detail);
        let turb = fbm_n(p.x * 5.0 + t * 0.32, p.y * 5.0, p.z * 5.0 - t * 0.22, o);
        let gran = if u.detail > 0.2 { noise(p.x * 22.0 - t * 0.7, p.y * 22.0, p.z * 22.0) } else { 0.5 };
        let wave = (p.y * 27.0 + turb * 17.0 + t * 1.4).sin() * 0.12;
        let heat = turb * 0.75 + gran * 0.25 + wave;
        let mut c = blend(rgb(98., 19., 44.), rgb(221., 48., 42.), smooth(0.27, 0.52, heat));
        c = blend(c, rgb(255., 143., 48.), smooth(0.49, 0.68, heat));
        c = blend(c, rgb(255., 241., 150.), smooth(0.67, 0.81, heat));
        let spots = 1.0 - smooth(0.17, 0.30, fbm_n(p.x * 8.0 + 21.0, p.y * 8.0, p.z * 8.0, o));
        c = blend(c, rgb(72., 25., 42.), spots * 0.64);
        // Limbo: usa la vista (no la luz), así sigue siendo independiente de light_dir.
        let facing = f.normal.dot(f.view_dir).max(0.0);
        finish(blend(c, rgb(255., 101., 43.), (1.0 - facing).powi(2) * 0.35))
    }
}
