//! Mapa de normales en el renderizador por software.
//!
//! Las texturas se GENERAN aquí (no hay archivos): un campo de altura periódico (ruido de valor con
//! retícula que envuelve, celdas de Worley periódicas) se convierte en normales tangentes por diferencias
//! finitas con envoltura. Por eso son tileables sin costura: la fila/columna 0 es vecina de la última.
//!
//! Decisión sobre la tangente: NO se añade `tangent` a `FragIn`. Los chunks del terreno traen uv de la cara
//! del cubo, que cambia de orientación en cada una de las 6 caras (costuras), y las esferas de los
//! planetas tienen uv polares (polo degenerado). En su lugar se usa mapeo triplanar con mezcla "whiteout":
//! para cada eje dominante de la normal (en espacio local, fijo al cuerpo) se proyecta la posición a ese
//! plano, se toma la normal tangente de esa proyección y su marco TBN es el del propio eje (T y B son los
//! otros dos ejes, N el eje). Los tres resultados se mezclan con pesos continuos |n|^4, así que no hay
//! costuras ni polos, y no hace falta calcular tangentes por triángulo en el rasterizador.
//!
//! Anti-centelleo: dos capas de escala (fina, 6 m por repetición; gruesa, 48 m) que se desvanecen con
//! `FragIn::distance`; desde el espacio el efecto vale 0 y no cuesta nada.
use super::noise::{hash, smooth};
use crate::math::Vec3;
use crate::raster::{FragIn, Uniforms};
use std::sync::OnceLock;

pub const SIZE: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Grava y roca suelta (Verdalia).
    Gravel,
    /// Columnas de basalto (Fornax).
    Basalt,
    /// Hielo agrietado (Nix).
    Ice,
    /// Escalones de píxel (Mutante).
    Steps,
}

pub struct NormalMap {
    /// Normales tangentes unitarias (x, y a lo largo de la textura, z hacia afuera), `SIZE * SIZE`.
    data: Vec<[f32; 3]>,
}

// ---------------------------------------------------------------- campo de altura periódico

fn wrap(i: i32, period: i32) -> i32 {
    i.rem_euclid(period)
}

/// Ruido de valor con retícula de `period` celdas que envuelve (tileable). `x`, `y` en celdas.
fn pnoise(x: f32, y: f32, period: i32, seed: i32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (ix, iy) = (fx as i32, fy as i32);
    let (tx, ty) = (smooth(0.0, 1.0, x - fx), smooth(0.0, 1.0, y - fy));
    let h = |dx: i32, dy: i32| hash(wrap(ix + dx, period), wrap(iy + dy, period), seed);
    let a = h(0, 0) + (h(1, 0) - h(0, 0)) * tx;
    let b = h(0, 1) + (h(1, 1) - h(0, 1)) * tx;
    a + (b - a) * ty
}

/// fbm periódico: cada octava duplica el periodo, así que el conjunto sigue envolviendo.
fn pfbm(u: f32, v: f32, base: i32, octaves: u32, seed: i32) -> f32 {
    let (mut s, mut amp, mut norm, mut per) = (0.0, 1.0, 0.0, base);
    for o in 0..octaves {
        s += amp * pnoise(u * per as f32, v * per as f32, per, seed + o as i32 * 17);
        norm += amp;
        amp *= 0.5;
        per *= 2;
    }
    s / norm
}

/// Worley periódico de `n` x `n` celdas: (distancia al punto más cercano, al segundo), en celdas.
fn worley(u: f32, v: f32, n: i32, seed: i32) -> (f32, f32) {
    let (x, y) = (u * n as f32, v * n as f32);
    let (cx, cy) = (x.floor() as i32, y.floor() as i32);
    let (mut d1, mut d2) = (f32::MAX, f32::MAX);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (gx, gy) = (cx + dx, cy + dy);
            let (hx, hy) = (wrap(gx, n), wrap(gy, n));
            let px = gx as f32 + 0.1 + 0.8 * hash(hx, hy, seed);
            let py = gy as f32 + 0.1 + 0.8 * hash(hx, hy, seed + 53);
            let d = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    (d1, d2)
}

fn height(surface: Surface, u: f32, v: f32) -> f32 {
    match surface {
        Surface::Gravel => {
            let (d1, _) = worley(u, v, 14, 3);
            let dome = (1.0 - (d1 / 0.62).min(1.0).powi(2)).max(0.0).sqrt();
            0.55 * dome + 0.45 * pfbm(u, v, 8, 4, 5)
        }
        Surface::Basalt => {
            let (d1, d2) = worley(u, v, 9, 11);
            let plateau = smooth(0.0, 0.16, d2 - d1);
            0.8 * plateau + 0.2 * pfbm(u, v, 16, 3, 7)
        }
        Surface::Ice => {
            let (d1, d2) = worley(u, v, 6, 23);
            let groove = 1.0 - smooth(0.0, 0.07, d2 - d1);
            0.35 * pfbm(u, v, 6, 3, 9) - 0.9 * groove
        }
        Surface::Steps => {
            // Bloques de 8x8 texels con 4 alturas; el desenfoque posterior da bordes de ~2 texels.
            let (cx, cy) = ((u * 32.0) as i32 % 32, (v * 32.0) as i32 % 32);
            (hash(cx, cy, 41) * 4.0).floor() / 4.0
        }
    }
}

fn strength(surface: Surface) -> f32 {
    match surface {
        Surface::Gravel => 3.2,
        Surface::Basalt => 3.0,
        Surface::Ice => 2.2,
        Surface::Steps => 1.6,
    }
}

impl NormalMap {
    pub fn generate(surface: Surface) -> Self {
        let n = SIZE;
        let mut h: Vec<f32> = (0..n * n).map(|i| height(surface, (i % n) as f32 / n as f32, (i / n) as f32 / n as f32)).collect();
        if surface == Surface::Steps {
            // desenfoque 3x3 con envoltura
            let src = h.clone();
            for y in 0..n {
                for x in 0..n {
                    let mut s = 0.0;
                    for dy in [n - 1, 0, 1] {
                        for dx in [n - 1, 0, 1] {
                            s += src[((y + dy) % n) * n + (x + dx) % n];
                        }
                    }
                    h[y * n + x] = s / 9.0;
                }
            }
        }
        let k = strength(surface) * n as f32 / 64.0;
        let at = |x: usize, y: usize| h[(y % n) * n + x % n];
        let data = (0..n * n)
            .map(|i| {
                let (x, y) = (i % n, i / n);
                // diferencias centrales con envoltura (x + n - 1 evita el desbordamiento en 0)
                let dx = (at(x + 1, y) - at(x + n - 1, y)) * 0.5;
                let dy = (at(x, y + 1) - at(x, y + n - 1)) * 0.5;
                let v = Vec3::new(-dx * k, -dy * k, 1.0).normalized();
                [v.x, v.y, v.z]
            })
            .collect();
        Self { data }
    }

    pub fn texel(&self, x: i32, y: i32) -> Vec3 {
        let t = self.data[wrap(y, SIZE as i32) as usize * SIZE + wrap(x, SIZE as i32) as usize];
        Vec3::new(t[0], t[1], t[2])
    }

    /// Muestreo bilineal con envoltura; `u`, `v` en repeticiones de textura (cualquier real).
    /// El resultado se renormaliza, así que siempre es unitario y finito.
    pub fn sample(&self, u: f32, v: f32) -> Vec3 {
        if !u.is_finite() || !v.is_finite() {
            return Vec3::new(0.0, 0.0, 1.0);
        }
        let (x, y) = (u * SIZE as f32 - 0.5, v * SIZE as f32 - 0.5);
        let (fx, fy) = (x.floor(), y.floor());
        let (tx, ty) = (x - fx, y - fy);
        let (ix, iy) = (fx as i32, fy as i32);
        let a = self.texel(ix, iy).lerp(self.texel(ix + 1, iy), tx);
        let b = self.texel(ix, iy + 1).lerp(self.texel(ix + 1, iy + 1), tx);
        let r = a.lerp(b, ty).normalized();
        if r == Vec3::ZERO { Vec3::new(0.0, 0.0, 1.0) } else { r }
    }
}

/// Texturas únicas por tipo de superficie, generadas al primer uso.
pub fn map(surface: Surface) -> &'static NormalMap {
    static MAPS: [OnceLock<NormalMap>; 4] = [OnceLock::new(), OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let i = surface as usize;
    MAPS[i].get_or_init(|| NormalMap::generate(surface))
}

// ---------------------------------------------------------------- perturbación de la normal

/// Parámetros de repetición y desvanecimiento por distancia.
#[derive(Clone, Copy)]
pub struct Tiling {
    /// Metros por repetición de la capa fina (la gruesa usa 8 veces más).
    pub tile_m: f32,
    /// Intensidad global del efecto (1 = la de la textura).
    pub strength: f32,
}

/// Peso de la capa fina y de la gruesa según la distancia a la cámara (m). Ambas valen 0 muy lejos.
pub fn layer_weights(distance: f32) -> (f32, f32) {
    (1.0 - smooth(60.0, 320.0, distance), 1.0 - smooth(450.0, 2600.0, distance))
}

/// Una capa triplanar con mezcla whiteout. `n` y `p` en espacio local, `p` en metros.
fn triplanar(map: &NormalMap, n: Vec3, p: Vec3, tile_m: f32) -> Vec3 {
    let inv = 1.0 / tile_m;
    let a = Vec3::new(n.x.abs().powi(4), n.y.abs().powi(4), n.z.abs().powi(4));
    let sum = (a.x + a.y + a.z).max(1e-12);
    let (wx, wy, wz) = (a.x / sum, a.y / sum, a.z / sum);
    let mut out = Vec3::ZERO;
    if wx > 0.02 {
        let t = map.sample(p.z * inv, p.y * inv);
        let tn = (t.x + n.z, t.y + n.y, t.z.abs() * n.x);
        out += Vec3::new(tn.2, tn.1, tn.0) * wx;
    }
    if wy > 0.02 {
        let t = map.sample(p.x * inv, p.z * inv);
        let tn = (t.x + n.x, t.y + n.z, t.z.abs() * n.y);
        out += Vec3::new(tn.0, tn.2, tn.1) * wy;
    }
    if wz > 0.02 {
        let t = map.sample(p.x * inv, p.y * inv);
        let tn = (t.x + n.x, t.y + n.y, t.z.abs() * n.z);
        out += Vec3::new(tn.0, tn.1, tn.2) * wz;
    }
    let r = out.normalized();
    if r == Vec3::ZERO { n } else { r }
}

/// Metros por unidad local: las esferas llevan el radio en el modelo, los chunks del terreno ya están en metros.
fn local_scale(u: &Uniforms) -> f32 {
    let m = &u.model.m;
    let s = (m[0][0] * m[0][0] + m[1][0] * m[1][0] + m[2][0] * m[2][0]).sqrt();
    if s.is_finite() && s > 1e-6 { s } else { 1.0 }
}

/// Normal perturbada en espacio LOCAL a partir de la normal local `n_local`. Con `enabled = false`
/// (o lejos) devuelve `n_local` sin cambios.
pub fn perturb_local_with(enabled: bool, map: &NormalMap, f: &FragIn, u: &Uniforms, n_local: Vec3, t: Tiling) -> Vec3 {
    if !enabled || t.strength <= 0.0 {
        return n_local;
    }
    let (w1, w2) = layer_weights(f.distance);
    if w1 <= 0.01 && w2 <= 0.01 {
        return n_local;
    }
    let p = f.p_local * local_scale(u);
    let mut n = n_local;
    if w1 > 0.01 {
        n += (triplanar(map, n_local, p, t.tile_m) - n_local) * (w1 * t.strength);
    }
    if w2 > 0.01 {
        n += (triplanar(map, n_local, p, t.tile_m * 8.0) - n_local) * (w2 * t.strength * 0.8);
    }
    let r = n.normalized();
    if r == Vec3::ZERO { n_local } else { r }
}

pub fn perturb_local(map: &NormalMap, f: &FragIn, u: &Uniforms, n_local: Vec3, t: Tiling) -> Vec3 {
    perturb_local_with(super::normal_map_enabled(), map, f, u, n_local, t)
}

/// Normal perturbada en espacio de MUNDO (la que usan los shaders que iluminan en mundo).
pub fn perturb_world(map: &NormalMap, f: &FragIn, u: &Uniforms, t: Tiling) -> Vec3 {
    if !super::normal_map_enabled() {
        return f.normal;
    }
    let nl = super::noise::to_local(&u.model, f.normal);
    let pl = perturb_local_with(true, map, f, u, nl, t);
    let w = u.model.transform_dir(pl).normalized();
    if w == Vec3::ZERO || !w.x.is_finite() { f.normal } else { w }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Mat4;

    const ALL: [Surface; 4] = [Surface::Gravel, Surface::Basalt, Surface::Ice, Surface::Steps];

    fn frag(p: Vec3, n: Vec3, dist: f32) -> FragIn {
        FragIn { p_local: p, normal: n, uv: [0.0; 2], color: [1.0; 3], view_dir: Vec3::Y, distance: dist }
    }
    fn uni(l: Vec3) -> Uniforms {
        Uniforms { light_dir: l.normalized(), time: 0.0, detail: 1.0, model: Mat4::IDENTITY }
    }
    const T: Tiling = Tiling { tile_m: 6.0, strength: 1.0 };

    #[test]
    fn textures_are_unit_and_tileable() {
        for s in ALL {
            let m = NormalMap::generate(s);
            for t in &m.data {
                let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
                assert!((l - 1.0).abs() < 1e-4 && t[2] > 0.0, "{s:?}");
            }
            // el muestreo continuo coincide a ambos lados del borde (envoltura)
            for i in 0..SIZE {
                let v = (i as f32 + 0.37) / SIZE as f32;
                let (a, b) = (m.sample(1e-6, v), m.sample(1.0 + 1e-6, v));
                assert!((a - b).length() < 1e-3, "{s:?} borde u");
                let (a, b) = (m.sample(v, 1e-6), m.sample(v, 1.0 + 1e-6));
                assert!((a - b).length() < 1e-3, "{s:?} borde v");
            }
            // el salto medio a través del borde es como el de cualquier otra columna de texels
            let mean = |x: i32| (0..SIZE as i32).map(|y| (m.texel(x, y) - m.texel(x + 1, y)).length()).sum::<f32>() / SIZE as f32;
            let (edge, inner) = (mean(SIZE as i32 - 1), (mean(100) + mean(37) + mean(200)) / 3.0);
            assert!(edge < inner * 1.6 + 0.02, "{s:?} borde {edge} interior {inner}");
            // no son planas
            let var: f32 = m.data.iter().map(|t| 1.0 - t[2]).sum::<f32>() / m.data.len() as f32;
            assert!(var > 0.005, "{s:?} demasiado plana: {var}");
        }
    }

    #[test]
    fn bilinear_is_bounded_deterministic_and_finite() {
        let m = map(Surface::Gravel);
        for i in -50..50 {
            let (u, v) = (i as f32 * 0.731, i as f32 * -1.37 + 0.2);
            let a = m.sample(u, v);
            assert_eq!(a, m.sample(u, v));
            assert!((a.length() - 1.0).abs() < 1e-4);
            assert!(a.z > 0.0 && a.x.abs() <= 1.0 && a.y.abs() <= 1.0);
        }
        assert_eq!(m.sample(f32::NAN, 0.0), Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(m.sample(0.25, 0.75), m.sample(1.25, -0.25));
    }

    #[test]
    fn perturbation_changes_lighting_and_depends_on_light() {
        let u = uni(Vec3::Y);
        let mut changed = 0;
        let mut dep = 0;
        for i in 0..200 {
            let p = Vec3::new(i as f32 * 0.9, 3500.0, i as f32 * 0.37);
            let n = Vec3::new(0.05, 1.0, 0.02).normalized();
            let f = frag(p, n, 30.0);
            let pn = perturb_local_with(true, map(Surface::Gravel), &f, &u, n, T);
            assert!((pn.length() - 1.0).abs() < 1e-4 && pn.x.is_finite());
            let (l1, l2) = (Vec3::new(1.0, 0.2, 0.0).normalized(), Vec3::new(-0.3, 0.2, 1.0).normalized());
            if (pn.dot(l1) - n.dot(l1)).abs() > 0.02 {
                changed += 1;
            }
            if ((pn.dot(l1) - n.dot(l1)) - (pn.dot(l2) - n.dot(l2))).abs() > 0.02 {
                dep += 1;
            }
        }
        assert!(changed > 150 && dep > 100, "{changed} {dep}");
    }

    #[test]
    fn disabled_or_far_is_identity() {
        let u = uni(Vec3::Y);
        let n = Vec3::new(0.3, 0.9, 0.1).normalized();
        let near = frag(Vec3::new(5.0, 3500.0, 2.0), n, 30.0);
        assert_eq!(perturb_local_with(false, map(Surface::Ice), &near, &u, n, T), n);
        let far = frag(Vec3::new(5.0, 3500.0, 2.0), n, 50_000.0);
        assert_eq!(perturb_local_with(true, map(Surface::Ice), &far, &u, n, T), n);
        // continuidad del desvanecimiento
        let (a, b) = (layer_weights(100.0), layer_weights(101.0));
        assert!((a.0 - b.0).abs() < 0.05 && (a.1 - b.1).abs() < 0.05);
    }

    #[test]
    fn no_seams_between_axis_regions() {
        // Recorre normales de la esfera: la normal perturbada varía de forma continua (sin saltos en las
        // fronteras donde cambia el eje dominante).
        let u = uni(Vec3::Y);
        let m = map(Surface::Basalt);
        let p = Vec3::new(10.0, 20.0, 30.0);
        let mut prev: Option<Vec3> = None;
        for i in 0..=2000 {
            let a = i as f32 / 2000.0 * 1.2 + 0.2;
            let n = Vec3::new(a.cos(), a.sin(), 0.3 * (a * 3.0).sin()).normalized();
            let r = perturb_local_with(true, m, &frag(p, n, 20.0), &u, n, T);
            if let Some(q) = prev {
                assert!((r - q).length() < 0.2, "salto {}", (r - q).length());
            }
            prev = Some(r);
        }
    }
}
