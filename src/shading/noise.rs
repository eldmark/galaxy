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

/// floor rápido (sin llamada a libm) para valores dentro del rango de i32.
#[inline(always)]
pub fn ifloor(x: f32) -> i32 {
    let i = x as i32;
    i - ((i as f32) > x) as i32
}

/// Valor del hash entero 3D a partir de los productos por eje ya calculados.
#[inline(always)]
fn hash_parts(a: u32, b: u32, c: u32) -> f32 {
    let mut n = a.wrapping_add(b).wrapping_add(c);
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) >> 8) as f32 * (1.0 / 16_777_215.0)
}

/// Las 8 esquinas de la celda de ruido (ix, iy, iz). Se pueden reutilizar mientras los puntos
/// consecutivos caigan en la misma celda (parallax, bump), sin cambiar el resultado.
#[derive(Clone, Copy)]
pub struct Corners {
    ix: i32,
    iy: i32,
    iz: i32,
    v: [f32; 8],
}
impl Corners {
    pub const EMPTY: Corners = Corners { ix: i32::MIN, iy: i32::MIN, iz: i32::MIN, v: [0.0; 8] };
    #[inline(always)]
    fn load(&mut self, ix: i32, iy: i32, iz: i32) {
        // Productos por eje una sola vez (6 multiplicaciones en vez de 24).
        let xs = [(ix as u32).wrapping_mul(374_761_393), (ix.wrapping_add(1) as u32).wrapping_mul(374_761_393)];
        let ys = [(iy as u32).wrapping_mul(668_265_263), (iy.wrapping_add(1) as u32).wrapping_mul(668_265_263)];
        let zs = [(iz as u32).wrapping_mul(2_147_483_647), (iz.wrapping_add(1) as u32).wrapping_mul(2_147_483_647)];
        self.ix = ix;
        self.iy = iy;
        self.iz = iz;
        for k in 0..8 {
            // Orden: x0 {y0z0, y1z0, y0z1, y1z1}, luego x1 igual (permite interpolar en x de 4 en 4).
            self.v[k] = hash_parts(xs[k >> 2], ys[k & 1], zs[(k >> 1) & 1]);
        }
    }
}

/// Ruido de valor 3D suave, en 0..1, reutilizando las esquinas de `c` si el punto cae en su celda.
#[inline]
pub fn noise_c(c: &mut Corners, x: f32, y: f32, z: f32) -> f32 {
    let (ix, iy, iz) = (ifloor(x), ifloor(y), ifloor(z));
    if c.ix != ix || c.iy != iy || c.iz != iz {
        c.load(ix, iy, iz);
    }
    let (tx, ty, tz) = (x - ix as f32, y - iy as f32, z - iz as f32);
    let fx = tx * tx * (3.0 - 2.0 * tx);
    let fy = ty * ty * (3.0 - 2.0 * ty);
    let fz = tz * tz * (3.0 - 2.0 * tz);
    let v = &c.v;
    let mut q = [0.0f32; 4];
    for k in 0..4 {
        q[k] = v[k] + (v[k + 4] - v[k]) * fx;
    }
    let (a, b) = (q[0] + (q[1] - q[0]) * fy, q[2] + (q[3] - q[2]) * fy);
    (a + (b - a) * fz).clamp(0.0, 1.0)
}

/// Ruido de valor 3D suave, en 0..1.
#[inline]
pub fn noise(x: f32, y: f32, z: f32) -> f32 {
    noise_c(&mut Corners::EMPTY.clone(), x, y, z)
}

/// Esquinas de las 3 octavas de un fbm, para reutilizarlas entre puntos cercanos.
#[derive(Clone, Copy)]
pub struct FbmCache([Corners; 3]);
impl FbmCache {
    pub const EMPTY: FbmCache = FbmCache([Corners::EMPTY; 3]);
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
/// Igual que `fbm_n` pero reutiliza (y actualiza) las esquinas de `cache`.
#[inline]
pub fn fbm_c(cache: &mut FbmCache, x: f32, y: f32, z: f32, oct: u32) -> f32 {
    let mut s = 0.57 * noise_c(&mut cache.0[0], x, y, z);
    let mut w = 0.57;
    if oct >= 2 {
        s += 0.29 * noise_c(&mut cache.0[1], x * 2.11, y * 2.11, z * 2.11);
        w += 0.29;
    }
    if oct >= 3 {
        s += 0.14 * noise_c(&mut cache.0[2], x * 4.31, y * 4.31, z * 4.31);
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
