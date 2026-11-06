//! Shaders de fragmentos propios. Dueño: agente `shading`.
//!
//! Todos devuelven RGB en 0..1 y usan `FragIn::p_local` normalizado como dirección sobre
//! la esfera (el patrón rota con el planeta). `Uniforms::detail` (0..1) reduce octavas de fbm.
//! El tablero de Conway/HighLife es una línea de tiempo precalculada de solo lectura
//! (ver `life.rs`): función pura del tiempo de simulación, sin `update`.
use crate::raster::{FragIn, FragmentShader, Uniforms};
use crate::world::BodyKind;

mod giant;
mod ice;
mod life;
mod noise;
mod pixel;
mod props;
mod rocky;
mod sky;
mod sun;
mod volcanic;

pub use giant::GiantShader;
pub use ice::IceShader;
pub use pixel::PixelShader;
pub use props::{PropShader, RingShader, ShipShader};
pub use rocky::RockyShader;
pub use sky::sky_color;
pub use sun::SunShader;
pub use volcanic::VolcanicShader;

/// Lambert simple, solo para que el esqueleto funcione antes de portar los shaders reales.
pub struct Fallback;

impl FragmentShader for Fallback {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let l = f.normal.dot(u.light_dir).max(0.0) * 0.9 + 0.1;
        [f.color[0] * l, f.color[1] * l, f.color[2] * l]
    }
}

/// Devuelve el shader de cada cuerpo.
pub fn shader_for(kind: BodyKind) -> Box<dyn FragmentShader> {
    match kind {
        BodyKind::Sun => Box::new(SunShader),
        BodyKind::Verdalia => Box::new(RockyShader::new()),
        BodyKind::Siroco => Box::new(GiantShader),
        BodyKind::Mutante => Box::new(PixelShader::new()),
        BodyKind::Nix => Box::new(IceShader),
        BodyKind::Fornax => Box::new(VolcanicShader::new()),
    }
}

/// Shader del anillo de Siroco (`uv.x` = radio normalizado 0..1).
pub fn shader_for_ring() -> Box<dyn FragmentShader> {
    Box::new(RingShader)
}

/// Shader de la nave del jugador.
pub fn ship_shader() -> Box<dyn FragmentShader> {
    Box::new(ShipShader)
}

/// Shader de los objetos de superficie (árboles, espiras, cristales, edificios).
pub fn prop_shader() -> Box<dyn FragmentShader> {
    Box::new(PropShader)
}

#[cfg(test)]
mod tests;
