//! Fondo del espacio: negro con estrellas deterministas (hash de la dirección), sin parpadeo.
use super::noise::*;
use crate::math::Vec3;

/// Una capa de estrellas: la esfera se divide en celdas 3D de tamaño 1/`scale`; cada celda
/// puede contener una estrella con posición, brillo y tinte por hash.
fn layer(d: Vec3, scale: f32, density: f32, seed: i32, radius: f32) -> Rgb {
    let q = d * scale;
    let (cx, cy, cz) = (q.x.floor() as i32, q.y.floor() as i32, q.z.floor() as i32);
    let mut out = [0.0f32; 3];
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (x, y, z) = (cx + dx, cy + dy, cz + dz);
                if hash(x, y, z + seed) > density {
                    continue;
                }
                let s = Vec3::new(
                    x as f32 + hash(x + 7, y, z + seed),
                    y as f32 + hash(x, y + 13, z + seed),
                    z as f32 + hash(x, y, z + seed + 29),
                );
                // La estrella es un punto sobre la esfera: distancia del rayo a su posición.
                let dist = (q - s).length();
                let a = 1.0 - smooth(0.0, radius, dist);
                if a <= 0.0 {
                    continue;
                }
                let b = hash(x + 3, y + 5, z + seed + 11);
                let bright = 0.25 + 0.75 * b * b * b + if b > 0.97 { 0.3 } else { 0.0 };
                let tint = hash(x + 17, y + 19, z + seed + 23);
                let col = if tint < 0.2 {
                    rgb(255., 190., 150.)
                } else if tint > 0.8 {
                    rgb(170., 200., 255.)
                } else {
                    rgb(255., 250., 240.)
                };
                for i in 0..3 {
                    out[i] += col[i] * bright * a;
                }
            }
        }
    }
    out
}

/// Color del fondo en la dirección `dir` (mundo). `time` se acepta por contrato pero las
/// estrellas son estáticas (sin parpadeo).
pub fn sky_color(dir: Vec3, _time: f32) -> [f32; 3] {
    let d = dir.normalized();
    if d == Vec3::ZERO {
        return [0.0; 3];
    }
    let a = layer(d, 90.0, 0.10, 0, 0.30); // pocas, más grandes
    let b = layer(d, 210.0, 0.14, 101, 0.38); // medias
    let c = layer(d, 480.0, 0.20, 307, 0.45); // polvo
    // Velo muy tenue tipo vía láctea.
    let band = 1.0 - smooth(0.0, 0.45, (d.x * 0.5 + d.y * 0.7 + d.z * 0.3).abs());
    let haze = fbm_n(d.x * 3.0 + 40.0, d.y * 3.0, d.z * 3.0, 2);
    let glow = band * smooth(0.45, 0.8, haze) * 0.05;
    finish([
        a[0] + b[0] + c[0] + glow * 0.55,
        a[1] + b[1] + c[1] + glow * 0.6,
        a[2] + b[2] + c[2] + glow * 0.9,
    ])
}
