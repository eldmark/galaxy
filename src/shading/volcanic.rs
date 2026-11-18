//! Fornax: placas de basalto (celdas 3D) con magma animado, bump mapping y parallax.
//! La iluminación se calcula en espacio local (luz/vista/normal transformadas con el modelo).
use super::noise::*;
use super::normalmap::{self, Surface, Tiling};
use crate::math::Vec3;
use crate::raster::{FragIn, FragmentShader, Uniforms};

pub struct VolcanicShader {
    pub bump: bool,
    pub parallax: bool,
}
impl VolcanicShader {
    pub fn new() -> Self {
        Self { bump: true, parallax: true }
    }
}
impl Default for VolcanicShader {
    fn default() -> Self {
        Self::new()
    }
}

struct Cell {
    border: f32,
    grad: Vec3,
}
/// Distancia mínima en un eje a la caja [d+0.15, d+0.85] donde cae el punto de la celda vecina `d`.
#[inline]
fn axis_gap(f: f32, d: i32) -> f32 {
    let (lo, hi) = (d as f32 + 0.15, d as f32 + 0.85);
    (lo - f).max(f - hi).max(0.0)
}
#[inline]
fn feature(x: i32, y: i32, z: i32) -> Vec3 {
    Vec3::new(
        x as f32 + 0.15 + hash(x, y, z) * 0.7,
        y as f32 + 0.15 + hash(x + 31, y, z) * 0.7,
        z as f32 + 0.15 + hash(x, y + 37, z) * 0.7,
    )
}
/// Puntos de las 27 celdas vecinas de una celda base, calculados bajo demanda y reutilizados
/// mientras los siguientes puntos caigan en la misma celda base.
#[derive(Clone, Copy)]
struct CellCache {
    base: [i32; 3],
    valid: u32,
    feats: [Vec3; 27],
}
impl CellCache {
    const EMPTY: CellCache = CellCache { base: [i32::MIN; 3], valid: 0, feats: [Vec3::ZERO; 27] };
}

fn cell_surface(p: Vec3, cache: &mut CellCache) -> Cell {
    let base = [ifloor(p.x), ifloor(p.y), ifloor(p.z)];
    if cache.base != base {
        cache.base = base;
        cache.valid = 0;
    }
    // Distancias mínimas por eje a cada columna de celdas (-1, 0, +1), al cuadrado.
    let gaps = |f: f32| [axis_gap(f, -1), axis_gap(f, 0), axis_gap(f, 1)].map(|g| g * g);
    let (gx, gy, gz) = (gaps(p.x - base[0] as f32), gaps(p.y - base[1] as f32), gaps(p.z - base[2] as f32));
    let (mut nearest, mut second) = (f32::INFINITY, f32::INFINITY);
    let (mut c1, mut c2) = (Vec3::ZERO, Vec3::ZERO);
    // La celda central primero: fija pronto un `second` pequeño y poda casi todas las demás.
    // Después, primero el lado más cercano de cada eje.
    let order = |f: f32| if f < 0.5 { [0, -1, 1] } else { [0, 1, -1] };
    let (ox, oy, oz) = (order(p.x - base[0] as f32), order(p.y - base[1] as f32), order(p.z - base[2] as f32));
    for &dz in &oz {
        let bz = gz[(dz + 1) as usize];
        if bz >= second {
            continue; // todo el plano queda más lejos que la segunda distancia
        }
        for &dy in &oy {
            let byz = bz + gy[(dy + 1) as usize];
            if byz >= second {
                continue;
            }
            for &dx in &ox {
                let bound = byz + gx[(dx + 1) as usize];
                if bound >= second {
                    continue; // ningún punto de esta celda puede entrar entre los dos más cercanos
                }
                let slot = ((dz + 1) * 9 + (dy + 1) * 3 + dx + 1) as usize;
                if cache.valid & (1 << slot) == 0 {
                    cache.feats[slot] = feature(base[0] + dx, base[1] + dy, base[2] + dz);
                    cache.valid |= 1 << slot;
                }
                let c = cache.feats[slot];
                let d = (p - c).length_sq();
                if d < nearest {
                    second = nearest;
                    c2 = c1;
                    nearest = d;
                    c1 = c;
                } else if d < second {
                    second = d;
                    c2 = c;
                }
            }
        }
    }
    let d1 = nearest.sqrt().max(1e-6);
    let d2 = second.sqrt().max(1e-6);
    Cell { border: (d2 - d1).max(0.0), grad: (p - c2) * (1.0 / d2) - (p - c1) * (1.0 / d1) }
}

#[cfg(test)]
fn cell_surface_brute(p: Vec3) -> Cell {
    let base = [ifloor(p.x), ifloor(p.y), ifloor(p.z)];
    let (mut nearest, mut second) = (f32::INFINITY, f32::INFINITY);
    let (mut c1, mut c2) = (Vec3::ZERO, Vec3::ZERO);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = feature(base[0] + dx, base[1] + dy, base[2] + dz);
                let d = (p - c).length_sq();
                if d < nearest {
                    second = nearest;
                    c2 = c1;
                    nearest = d;
                    c1 = c;
                } else if d < second {
                    second = d;
                    c2 = c;
                }
            }
        }
    }
    let d1 = nearest.sqrt().max(1e-6);
    let d2 = second.sqrt().max(1e-6);
    Cell { border: (d2 - d1).max(0.0), grad: (p - c2) * (1.0 / d2) - (p - c1) * (1.0 / d1) }
}

#[cfg(test)]
mod prune_tests {
    use super::*;
    #[test]
    fn pruned_cell_surface_matches_brute_force() {
        let mut s: u32 = 12345;
        let mut rnd = || {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 8) as f32 / 16_777_216.0
        };
        for _ in 0..20_000 {
            let p = Vec3::new(rnd() * 80.0 - 40.0, rnd() * 80.0 - 40.0, rnd() * 80.0 - 40.0);
            let mut cache = CellCache::EMPTY;
            let (a, b) = (cell_surface(p, &mut cache), cell_surface_brute(p));
            // segunda consulta con la caché caliente (y un punto cercano en la misma celda)
            let a2 = cell_surface(p, &mut cache);
            assert_eq!(a.border, a2.border);
            assert!((a.border - b.border).abs() < 1e-5, "border {:?}", p);
            assert!((a.grad - b.grad).length() < 1e-5, "grad {:?}", p);
        }
    }
}

struct Plate {
    flow: f32,
    rock: f32,
    height: f32,
    crack: f32,
    slope: Vec3,
}
/// Cachés de una evaluación de `plate`. Cada una de las 3 evaluaciones por píxel (parallax x2 y
/// principal) tiene la suya: los píxeles vecinos las reutilizan casi siempre.
#[derive(Clone, Copy)]
struct Caches {
    flow: FbmCache,
    rock: FbmCache,
    cell: CellCache,
}
impl Caches {
    const EMPTY: Caches = Caches { flow: FbmCache::EMPTY, rock: FbmCache::EMPTY, cell: CellCache::EMPTY };
}
#[inline(always)]
fn plate(p: Vec3, t: f32, o: u32, cc: &mut Caches) -> Plate {
    let flow = fbm_c(&mut cc.flow, p.x * 3.5 + t * 0.12, p.y * 3.5, p.z * 3.5, o);
    let w = Vec3::new(p.x * 7.0 + flow * 0.55, p.y * 7.0, p.z * 7.0 - flow * 0.35);
    let cell = cell_surface(w, &mut cc.cell);
    let plateau = smooth(0.03, 0.13, cell.border);
    let rock = fbm_c(&mut cc.rock, p.x * 13.0, p.y * 13.0, p.z * 13.0, o);
    let tt = (cell.border - 0.03) / 0.10;
    let k = if (0.0..1.0).contains(&tt) { 0.55 * 6.0 * tt * (1.0 - tt) / 0.10 * 7.0 } else { 0.0 };
    Plate { flow, rock, crack: 1.0 - plateau, height: 0.25 + 0.55 * plateau + 0.20 * rock, slope: cell.grad * k }
}

fn parallax_pos(p: Vec3, n: Vec3, view: Vec3, t: f32, o: u32, cc: &mut [Caches; 3]) -> Vec3 {
    let h = plate(p, t, o, &mut cc[0]).height;
    let facing = n.dot(view).max(0.35);
    let tangent = view - n * n.dot(view);
    let dist = |h: f32| ((1.0 - h) * 0.03 / facing).min(0.06);
    let first = p - tangent * dist(h);
    let h2 = plate(first, t, o, &mut cc[1]).height;
    p - tangent * dist((h + h2) * 0.5)
}

thread_local! {
    /// Esquinas de ruido persistentes por hilo: los píxeles vecinos caen casi siempre en las mismas
    /// celdas de ruido, así que casi nunca hay que recalcular hashes. Solo memoiza (resultado idéntico).
    static CACHES: std::cell::RefCell<[Caches; 3]> = const { std::cell::RefCell::new([Caches::EMPTY; 3]) };
}

impl FragmentShader for VolcanicShader {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        CACHES.with(|c| self.shade_with(f, u, &mut c.borrow_mut()))
    }
}

impl VolcanicShader {
    fn shade_with(&self, f: &FragIn, u: &Uniforms, cc: &mut [Caches; 3]) -> [f32; 3] {
        let p = sphere_dir(f.p_local);
        let o = octaves(u.detail);
        let lite = u.detail >= 0.45; // por debajo: sin parallax ni bump
        let n = to_local(&u.model, f.normal);
        let light = light_local(u);
        let view = to_local(&u.model, f.view_dir);
        let sample = if self.parallax && lite { parallax_pos(p, n, view, u.time, o, cc) } else { p };
        let s = plate(sample, u.time, o, &mut cc[2]);
        let mut c = blend(rgb(27., 26., 39.), rgb(86., 55., 62.), smooth(0.28, 0.72, s.rock));
        c = blend(c, rgb(159., 49., 47.), s.crack * 0.67);
        c = blend(c, rgb(255., 113., 38.), s.crack * smooth(0.38, 0.61, s.flow));
        c = blend(c, rgb(255., 219., 111.), s.crack * smooth(0.62, 0.78, s.flow) * 0.65);
        let ln = if self.bump && lite {
            let eps = 0.012;
            let mut rs = [0.0f32; 3];
            let base = [sample.x, sample.y, sample.z];
            for a in 0..3 {
                let mut q = base;
                q[a] += eps;
                // Copia de la caché del punto `sample`: el punto desplazado casi siempre cae en la misma celda.
                let mut rc = cc[2].rock;
                rs[a] = 0.20 * (fbm_c(&mut rc, q[0] * 13.0, q[1] * 13.0, q[2] * 13.0, o) - s.rock) / eps;
            }
            let slope = s.slope * 0.014 + Vec3::new(rs[0], rs[1], rs[2]) * 0.10;
            let tang = slope - n * slope.dot(n);
            (n - tang).normalized()
        } else {
            n
        };
        // Complemento del bump procedural: normales de columnas de basalto (más fino y estable de cerca).
        let ln = normalmap::perturb_local(normalmap::map(Surface::Basalt), f, u, ln, Tiling { tile_m: 6.0, strength: 1.0 });
        let diffuse = 0.20 + 0.8 * ln.dot(light).max(0.0);
        let half = (light + view).normalized();
        let spec = ln.dot(half).max(0.0).powi(24) * (1.0 - s.crack) * (36.0 / 255.0);
        let lit_c = add_all(lit(c, diffuse), spec);
        // Las grietas emiten luz incluso en el lado oscuro.
        finish(blend(lit_c, c, s.crack * 0.7))
    }
}
