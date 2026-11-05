//! Relieve de cada planeta: UNA función pura y determinista que usan la malla (deformación por ruido),
//! el terreno con LOD, la colisión y la distribución de objetos. Dueño: agente `relief`.
//!
//! Ruido de valor 3D propio (copia de `shading::noise` con semillas distintas por cuerpo, para no
//! acoplar el módulo). Verdalia usa EXACTAMENTE el mismo fbm de baja frecuencia que su shader
//! (`fbm_n(p*3.5 + (4,0,0), 3 octavas)`, umbrales 0.44/0.54 del suelo), así que los continentes
//! pintados coinciden con la tierra elevada y los océanos con el fondo aplanado.
//!
//! Amplitudes (relativas al radio): Verdalia ±3 %, Fornax ±3.5 %, Nix ±2 %, Mutante ±2.5 %
//! (16 escalones), Sol y Siroco 0.
use crate::geometry::SphereLods;
use crate::math::Vec3;
use crate::world::BodyKind;

const AMP_VERDALIA: f32 = 0.030;
const AMP_FORNAX: f32 = 0.035;
const AMP_NIX: f32 = 0.020;
const AMP_MUTANTE: f32 = 0.025;
/// Escalones de la altura de Mutante.
pub const MUTANTE_STEPS: u32 = 16;

// ---- ruido de valor 3D (idéntico en forma al de shading::noise) ----
fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut n = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add((z as u32).wrapping_mul(2_147_483_647));
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) >> 8) as f32 / 16_777_215.0
}
fn sm(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn smooth(a: f32, b: f32, x: f32) -> f32 {
    sm((x - a) / (b - a))
}
fn noise(x: f32, y: f32, z: f32) -> f32 {
    let (fx0, fy0, fz0) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx0 as i32, fy0 as i32, fz0 as i32);
    let (fx, fy, fz) = (sm(x - fx0), sm(y - fy0), sm(z - fz0));
    let a = lerp(hash(ix, iy, iz), hash(ix + 1, iy, iz), fx);
    let b = lerp(hash(ix, iy + 1, iz), hash(ix + 1, iy + 1, iz), fx);
    let c = lerp(hash(ix, iy, iz + 1), hash(ix + 1, iy, iz + 1), fx);
    let d = lerp(hash(ix, iy + 1, iz + 1), hash(ix + 1, iy + 1, iz + 1), fx);
    lerp(lerp(a, b, fy), lerp(c, d, fy), fz).clamp(0.0, 1.0)
}
/// fbm de `oct` (1..=4) octavas, normalizado a 0..1 (pesos 0.57/0.29/0.14/0.07).
fn fbm(p: Vec3, freq: f32, off: Vec3, oct: u32) -> f32 {
    let (x, y, z) = (p.x * freq + off.x, p.y * freq + off.y, p.z * freq + off.z);
    let mut s = 0.57 * noise(x, y, z);
    let mut w = 0.57;
    if oct >= 2 {
        s += 0.29 * noise(x * 2.11, y * 2.11, z * 2.11);
        w += 0.29;
    }
    if oct >= 3 {
        s += 0.14 * noise(x * 4.31, y * 4.31, z * 4.31);
        w += 0.14;
    }
    if oct >= 4 {
        s += 0.07 * noise(x * 8.7, y * 8.7, z * 8.7);
        w += 0.07;
    }
    s / w
}

/// Desplazamiento radial RELATIVO al radio del cuerpo en la dirección unitaria `dir`
/// (el radio real de la superficie es `radius * (1.0 + height(kind, dir))`). Continua (salvo los
/// escalones deliberados de Mutante), determinista y barata (<= 5 octavas de ruido).
/// `dir` está en el marco FIJO al cuerpo (el mismo que `p_local` del shader). Sol y Siroco devuelven 0.
pub fn height(kind: BodyKind, dir: Vec3) -> f32 {
    let d = dir.normalized();
    if d == Vec3::ZERO {
        return 0.0;
    }
    let h = match kind {
        BodyKind::Sun | BodyKind::Siroco => return 0.0,
        BodyKind::Verdalia => {
            // Mismo ruido y escala que el shader: continentes donde el shader pinta tierra.
            let c = fbm(d, 3.5, Vec3::new(4.0, 0.0, 0.0), 3);
            let land = smooth(0.44, 0.54, c);
            // fondo marino casi plano (-0.35 con leve pendiente), tierra sube hasta 1 al interior.
            let sea = -0.35 - 0.25 * ((0.44 - c) / 0.2).clamp(0.0, 1.0);
            let inland = ((c - 0.54) / 0.2).clamp(0.0, 1.0);
            let base = lerp(sea, 0.0, land) + 0.75 * inland;
            // colinas finas solo en tierra
            let det = fbm(d, 14.0, Vec3::new(1.7, 5.3, 2.9), 3) - 0.5;
            AMP_VERDALIA * (base + 0.5 * det * land).clamp(-1.0, 1.0)
        }
        BodyKind::Fornax => {
            // crestas (1-|2n-1|)^2 y placas escalonadas suaves de baja frecuencia
            let n = fbm(d, 4.0, Vec3::new(9.1, 2.2, 7.7), 4);
            let ridge = 1.0 - (2.0 * n - 1.0).abs();
            let plates = smooth(0.35, 0.65, fbm(d, 1.8, Vec3::new(3.3, 8.8, 1.1), 2));
            let v = 0.65 * ridge * ridge + 0.35 * plates;
            AMP_FORNAX * (2.0 * v - 0.8).clamp(-1.0, 1.0)
        }
        BodyKind::Nix => {
            // mesetas suaves: sigmoide sobre fbm de baja frecuencia + leve textura
            let n = fbm(d, 2.6, Vec3::new(6.4, 1.3, 5.9), 3);
            let mesa = smooth(0.40, 0.60, n);
            let det = fbm(d, 9.0, Vec3::new(0.4, 7.7, 3.1), 2) - 0.5;
            AMP_NIX * (0.8 * (2.0 * mesa - 1.0) + 0.3 * det).clamp(-1.0, 1.0)
        }
        BodyKind::Mutante => {
            let n = fbm(d, 3.0, Vec3::new(2.5, 6.1, 8.3), 3);
            let k = ((n * MUTANTE_STEPS as f32).floor() as i32).clamp(0, MUTANTE_STEPS as i32 - 1);
            let q = k as f32 / (MUTANTE_STEPS - 1) as f32; // 16 valores 0..1
            AMP_MUTANTE * (2.0 * q - 1.0)
        }
    };
    if h.is_finite() { h } else { 0.0 }
}

/// Cota superior de `height().abs()` para ese cuerpo (sirve para culling y colisión conservadores).
pub fn max_amplitude(kind: BodyKind) -> f32 {
    match kind {
        BodyKind::Sun | BodyKind::Siroco => 0.0,
        BodyKind::Verdalia => AMP_VERDALIA,
        BodyKind::Fornax => AMP_FORNAX,
        BodyKind::Nix => AMP_NIX,
        BodyKind::Mutante => AMP_MUTANTE,
    }
}

/// Esferas con LOD deformadas con el relieve del cuerpo (reemplazan a las esferas lisas).
/// Sol y Siroco salen lisas (altura 0).
pub fn displaced_lods(kind: BodyKind, max_subdiv: u32) -> SphereLods {
    SphereLods::new(max_subdiv, &|d| height(kind, d))
}

#[cfg(test)]
mod tests;
