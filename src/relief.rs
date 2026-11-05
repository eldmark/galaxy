//! Relieve de cada planeta: UNA función pura y determinista que usan la malla (deformación por ruido),
//! el terreno con LOD, la colisión y la distribución de objetos. Dueño: agente `relief`.
use crate::math::Vec3;
use crate::world::BodyKind;

/// Desplazamiento radial RELATIVO al radio del cuerpo en la dirección unitaria `dir`
/// (el radio real de la superficie es `radius * (1.0 + height(kind, dir))`). Debe ser continua,
/// determinista y barata. Sol y Siroco (sin suelo) devuelven 0.
pub fn height(_kind: BodyKind, _dir: Vec3) -> f32 {
    0.0
}

/// Cota superior de `height().abs()` para ese cuerpo (sirve para culling y colisión conservadores).
pub fn max_amplitude(_kind: BodyKind) -> f32 {
    0.0
}
