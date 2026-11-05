//! Shaders de fragmentos propios. Dueño: agente `shading`.
use crate::raster::{FragIn, FragmentShader, Uniforms};
use crate::world::BodyKind;

/// Lambert simple, solo para que el esqueleto funcione antes de portar los shaders reales.
pub struct Fallback;

impl FragmentShader for Fallback {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let l = f.normal.dot(u.light_dir).max(0.0) * 0.9 + 0.1;
        [f.color[0] * l, f.color[1] * l, f.color[2] * l]
    }
}

/// Devuelve el shader de cada cuerpo.
pub fn shader_for(_kind: BodyKind) -> Box<dyn FragmentShader> {
    Box::new(Fallback)
}
