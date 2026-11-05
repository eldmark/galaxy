//! Ruido de valor 3D, fbm con octavas variables (LOD de shader) y utilidades de color.
use crate::math::{Mat4, Vec3};
use crate::raster::Uniforms;

pub type Rgb = [f32; 3];

/// Color desde bytes 0..255 (paletas portadas de lab6) a 0..1.
pub fn rgb(r: f32, g: f32, b: f32) -> Rgb {
    [r / 255.0, g / 255.0, b / 255.0]
}
pub fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
pub fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    [mix(a[0], b[0], t), mix(a[1], b[1], t), mix(a[2], b[2], t)]
}
pub fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
pub fn lit(c: Rgb, k: f32) -> Rgb {
    [c[0] * k, c[1] * k, c[2] * k]
}
pub fn add_all(c: Rgb, v: f32) -> Rgb {
    [c[0] + v, c[1] + v, c[2] + v]
}
/// Garantiza finito y dentro de 0..1.
pub fn finish(c: Rgb) -> Rgb {
    c.map(|v| if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 })
}

/// Hash entero 3D -> 0..1.
pub fn hash(x: i32, y: i32, z: i32) -> f32 {
    let mut n = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add((z as u32).wrapping_mul(2_147_483_647));
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) >> 8) as f32 / 16_777_215.0
}

/// Ruido de valor 3D suave, en 0..1.
pub fn noise(x: f32, y: f32, z: f32) -> f32 {
    let (fx0, fy0, fz0) = (x.floor(), y.floor(), z.floor());
    let (ix, iy, iz) = (fx0 as i32, fy0 as i32, fz0 as i32);
    let fx = smooth(0.0, 1.0, x - fx0);
    let fy = smooth(0.0, 1.0, y - fy0);
    let fz = smooth(0.0, 1.0, z - fz0);
    let a = mix(hash(ix, iy, iz), hash(ix + 1, iy, iz), fx);
    let b = mix(hash(ix, iy + 1, iz), hash(ix + 1, iy + 1, iz), fx);
    let c = mix(hash(ix, iy, iz + 1), hash(ix + 1, iy, iz + 1), fx);
    let d = mix(hash(ix, iy + 1, iz + 1), hash(ix + 1, iy + 1, iz + 1), fx);
    mix(mix(a, b, fy), mix(c, d, fy), fz).clamp(0.0, 1.0)
}

/// Número de octavas (1..=3) según `detail` (0..1).
pub fn octaves(detail: f32) -> u32 {
    if detail >= 0.67 {
        3
    } else if detail >= 0.34 {
        2
    } else {
        1
    }
}

/// fbm de hasta 3 octavas (pesos de lab6), renormalizado para que la media no cambie con el LOD.
pub fn fbm_n(x: f32, y: f32, z: f32, oct: u32) -> f32 {
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
    s / w
}
pub fn fbm(x: f32, y: f32, z: f32) -> f32 {
    fbm_n(x, y, z, 3)
}

/// Dirección unitaria sobre la esfera a partir de la posición local.
pub fn sphere_dir(p: Vec3) -> Vec3 {
    let n = p.normalized();
    if n == Vec3::ZERO { Vec3::Z } else { n }
}

/// Vector de mundo -> espacio local (transpuesta de la parte rotacional del modelo).
pub fn to_local(m: &Mat4, v: Vec3) -> Vec3 {
    let r = Vec3::new(
        m.m[0][0] * v.x + m.m[1][0] * v.y + m.m[2][0] * v.z,
        m.m[0][1] * v.x + m.m[1][1] * v.y + m.m[2][1] * v.z,
        m.m[0][2] * v.x + m.m[1][2] * v.y + m.m[2][2] * v.z,
    )
    .normalized();
    if r == Vec3::ZERO { v } else { r }
}
pub fn light_local(u: &Uniforms) -> Vec3 {
    to_local(&u.model, u.light_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn noise_bounded_deterministic() {
        for i in -40..40 {
            let (x, y, z) = (i as f32 * 0.37, i as f32 * -0.91 + 0.2, i as f32 * 1.7);
            let v = noise(x, y, z);
            assert!((0.0..=1.0).contains(&v));
            assert_eq!(v, noise(x, y, z));
            for o in 1..=3 {
                assert!((0.0..=1.0).contains(&fbm_n(x, y, z, o)));
            }
        }
    }
}
