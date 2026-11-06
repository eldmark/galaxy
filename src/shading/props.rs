//! Anillo de Siroco y nave del jugador.
use super::noise::*;
use crate::raster::{FragIn, FragmentShader, Uniforms};

/// Anillo: `uv.x` es el radio normalizado 0..1 (borde interior -> exterior).
/// Franjas finas con grano, hueco oscuro (división) y tinte según el radio.
/// Sin canal alfa: el hueco se pinta casi negro.
pub struct RingShader;

impl FragmentShader for RingShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let r = f.uv[0].clamp(0.0, 1.0);
        let radius = 0.86 + r * 0.45; // escala de lab6 (0.86..1.31)
        let p = f.p_local;
        let grain = noise(p.x * 27.0, p.y * 27.0, p.z * 27.0);
        let stripe = (radius * 76.0 + grain * 1.8).sin() * 0.5 + 0.5;
        let gap = 1.0 - smooth(0.016, 0.036, (radius - 1.08).abs());
        let tint = smooth(0.87, 1.31, radius);
        let c = [
            (210.0 + 38.0 * stripe - 42.0 * tint) / 255.0,
            (177.0 + 47.0 * stripe - 28.0 * tint) / 255.0,
            (151.0 + 54.0 * stripe + 12.0 * tint) / 255.0,
        ];
        let density = (0.45 + stripe * 0.43) * (1.0 - gap * 0.9);
        // Dos caras iluminadas por igual: usa |N.L|.
        let d = f.normal.dot(u.light_dir).abs();
        finish(lit(c, density * (0.35 + 0.65 * d) + 0.04))
    }
}

/// Nave: Lambert + especular suave sobre el color de vértice, con una luz de relleno tenue.
pub struct ShipShader;

impl FragmentShader for ShipShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let nl = f.normal.dot(u.light_dir);
        let d = nl.max(0.0);
        let h = (u.light_dir + f.view_dir).normalized();
        let spec = if nl > 0.0 { f.normal.dot(h).max(0.0).powi(32) * 0.35 } else { 0.0 };
        let amb = 0.18;
        let k = amb + (1.0 - amb) * d;
        finish([f.color[0] * k + spec, f.color[1] * k + spec, f.color[2] * k + spec])
    }
}

/// Objetos de superficie (árboles, espiras, cristales, edificios): Lambert con luz ambiente 0.25 sobre el color de vértice.
pub struct PropShader;

impl FragmentShader for PropShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let d = f.normal.dot(u.light_dir).max(0.0);
        let k = 0.25 + 0.75 * d;
        finish([f.color[0] * k, f.color[1] * k, f.color[2] * k])
    }
}
