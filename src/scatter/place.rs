//! Distribución procedural de instancias sobre una celda de scatter (un chunk de nivel `CELL_LEVEL`).
//!
//! Receta (rúbrica: distribución procedural reproducible con semilla y ruido):
//! ```text
//! rng   = hash(semilla_global, cuerpo, cara, nivel, x, y)        // solo depende de la clave de la celda
//! rejilla de G x G casillas dentro de la celda, con jitter dado por el rng
//! por casilla (siempre se consumen los mismos 6 números aleatorios, se acepte o no):
//!     n = fbm(dir * freq + offset)                               // ruido de valor visto en clase
//!     acepta si rand < densidad * smoothstep(umbral, umbral + suave, n)
//!     y si la altura, la pendiente (normal del relieve) y la planitud de la huella lo permiten
//! ```
//! Como el rng sale de la clave de la celda y no de un estado global, la misma semilla da las mismas posiciones
//! sin importar el orden en que se generen los chunks.
use super::species::{Species, species_for};
use crate::math::{DVec3, Vec3};
use crate::terrain::chunk::{GRID, HeightFn};
use crate::terrain::cubesphere::{cross, face_uv_to_dir};
use crate::terrain::ChunkKey;
use crate::world::BodyKind;

/// Nivel del quadtree de las celdas donde se generan las instancias (coincide con el nivel máximo del terreno).
pub const CELL_LEVEL: u8 = 8;
/// Tope de instancias por celda (con 4x4 candidatos nunca se alcanza; protege las especies muy densas).
pub const MAX_PER_CELL: usize = 48;

/// Una instancia ya colocada, en el espacio LOCAL del planeta (gira y orbita con él).
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    /// Índice de la especie en la tabla del cuerpo.
    pub species: u8,
    /// Coordenadas de cara (u, v) del punto de apoyo (para filtrar por chunk).
    pub u: f64,
    pub v: f64,
    /// Dirección unitaria desde el centro del planeta.
    pub dir: DVec3,
    /// Punto de la base (m, local): `dir * radio_de_la_superficie - normal * hundimiento`.
    pub pos: DVec3,
    /// Normal del relieve en ese punto (eje Y local de la instancia).
    pub up: Vec3,
    /// Giro alrededor de `up` (rad).
    pub yaw: f32,
    pub scale: f32,
}

impl Instance {
    /// Base ortonormal derecha (x, y = up, z): columnas de la rotación de la instancia.
    /// Y es la normal del relieve; X y Z giran `yaw` alrededor de ella.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let n = self.up;
        let r = if n.y.abs() < 0.9 { Vec3::Y } else { Vec3::new(1.0, 0.0, 0.0) };
        let t = r.cross(n).normalized();
        let b = n.cross(t);
        let (s, c) = self.yaw.sin_cos();
        let x = t * c + b * s;
        let z = x.cross(n);
        (x, n, z)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Hash y rng deterministas (splitmix64)

fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn body_id(kind: BodyKind) -> u64 {
    match kind {
        BodyKind::Sun => 0,
        BodyKind::Verdalia => 1,
        BodyKind::Siroco => 2,
        BodyKind::Mutante => 3,
        BodyKind::Nix => 4,
        BodyKind::Fornax => 5,
    }
}

/// `hash(semilla_global, cuerpo, cara, nivel, x, y)`.
pub fn cell_hash(seed: u64, kind: BodyKind, key: ChunkKey) -> u64 {
    let mut h = mix(seed);
    for v in [body_id(kind), key.face as u64, key.level as u64, key.x as u64, key.y as u64] {
        h = mix(h ^ mix(v.wrapping_add(0x1234_5678)));
    }
    h
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 = mix(self.0);
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
}

// ---------------------------------------------------------------------------------------------------------
// Ruido de valor 3D con fbm (el mismo tipo de ruido que los shaders y el relieve)

fn lattice(x: i32, y: i32, z: i32) -> f32 {
    let mut n = (x as u32)
        .wrapping_mul(374_761_393)
        .wrapping_add((y as u32).wrapping_mul(668_265_263))
        .wrapping_add((z as u32).wrapping_mul(2_147_483_647));
    n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
    ((n ^ (n >> 16)) >> 8) as f32 / 16_777_215.0
}
fn smooth01(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn value_noise(p: Vec3) -> f32 {
    let (f0x, f0y, f0z) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (ix, iy, iz) = (f0x as i32, f0y as i32, f0z as i32);
    let (fx, fy, fz) = (smooth01(p.x - f0x), smooth01(p.y - f0y), smooth01(p.z - f0z));
    let a = lerp(lattice(ix, iy, iz), lattice(ix + 1, iy, iz), fx);
    let b = lerp(lattice(ix, iy + 1, iz), lattice(ix + 1, iy + 1, iz), fx);
    let c = lerp(lattice(ix, iy, iz + 1), lattice(ix + 1, iy, iz + 1), fx);
    let d = lerp(lattice(ix, iy + 1, iz + 1), lattice(ix + 1, iy + 1, iz + 1), fx);
    lerp(lerp(a, b, fy), lerp(c, d, fy), fz)
}
/// fbm de 3 octavas normalizado a 0..1 (pesos 0.57/0.29/0.14).
pub fn fbm3(p: Vec3) -> f32 {
    (0.57 * value_noise(p) + 0.29 * value_noise(p * 2.11) + 0.14 * value_noise(p * 4.31))
}

/// Factor 0..1 de la mancha de la especie en `dir` (0 fuera de la mancha).
pub fn cluster_weight(sp: &Species, dir: Vec3) -> f32 {
    let n = fbm3(dir * sp.noise_freq + sp.noise_offset);
    smooth01((n - sp.noise_threshold) / sp.noise_soft.max(1e-4))
}

// ---------------------------------------------------------------------------------------------------------

/// Generador de instancias de un cuerpo (puro: no guarda estado entre llamadas).
pub struct Placer {
    pub kind: BodyKind,
    pub radius: f64,
    pub seed: u64,
    pub species: Vec<Species>,
    height: Box<HeightFn>,
}

/// Cuántos candidatos por lado tiene una celda para una especie.
pub fn grid_per_side(cell_len_m: f64, cell_m: f32) -> usize {
    ((cell_len_m / cell_m as f64).round() as usize).clamp(1, 8)
}

impl Placer {
    pub fn new(kind: BodyKind, radius: f64, seed: u64) -> Self {
        Self { kind, radius, seed, species: species_for(kind), height: Box::new(move |d| crate::relief::height(kind, d)) }
    }

    /// Altura relativa del relieve (la misma función que usa el terreno).
    pub fn rel_height(&self, dir: DVec3) -> f64 {
        (self.height)(dir.to_f32()) as f64
    }
    /// Radio de la superficie (m) en una dirección unitaria.
    pub fn surface_radius(&self, dir: DVec3) -> f64 {
        self.radius * (1.0 + self.rel_height(dir))
    }

    /// Largo del lado (m) de una celda de nivel `CELL_LEVEL` sobre la esfera nominal.
    pub fn cell_len_m(&self) -> f64 {
        crate::terrain::chunk::chunk_size_m(&ChunkKey { face: 0, level: CELL_LEVEL, x: 0, y: 0 }, self.radius)
    }

    /// Instancias de una celda de nivel `CELL_LEVEL`. Determinista: función pura de (semilla, cuerpo, clave).
    pub fn cell_instances(&self, cell: ChunkKey) -> Vec<Instance> {
        debug_assert_eq!(cell.level, CELL_LEVEL);
        let mut out = Vec::new();
        let base_hash = cell_hash(self.seed, self.kind, cell);
        let (u0, u1) = (cell.u_at(0), cell.u_at(GRID));
        let (v0, v1) = (cell.v_at(0), cell.v_at(GRID));
        let len = self.cell_len_m();
        for (si, sp) in self.species.iter().enumerate() {
            // Cada especie tiene su propio flujo aleatorio: añadir una especie no mueve las demás.
            let mut rng = Rng(mix(base_hash ^ mix(si as u64 + 77)));
            let g = grid_per_side(len, sp.cell_m);
            let eps = sp.foot_m as f64 / (self.radius * std::f64::consts::FRAC_PI_4);
            for j in 0..g {
                for i in 0..g {
                    // Siempre los mismos 6 números, aceptada o no (el flujo no depende de los filtros).
                    let (rx, ry, rp, ryaw, rs, _spare) = (rng.next(), rng.next(), rng.next(), rng.next(), rng.next(), rng.next());
                    let jit = sp.jitter as f64;
                    let fu = (i as f64 + 0.5 + (rx as f64 - 0.5) * jit) / g as f64;
                    let fv = (j as f64 + 0.5 + (ry as f64 - 0.5) * jit) / g as f64;
                    let (u, v) = (u0 + (u1 - u0) * fu, v0 + (v1 - v0) * fv);
                    let dir = face_uv_to_dir(cell.face, u, v);
                    let w = cluster_weight(sp, dir.to_f32());
                    if rp >= sp.density * w {
                        continue;
                    }
                    if let Some(inst) = self.settle(sp, si as u8, cell.face, u, v, dir, eps, ryaw, rs) {
                        out.push(inst);
                        if out.len() >= MAX_PER_CELL {
                            return out;
                        }
                    }
                }
            }
        }
        out
    }

    /// Filtros de relieve (altitud, pendiente, planitud) y cálculo de posición/normal.
    #[allow(clippy::too_many_arguments)]
    fn settle(&self, sp: &Species, si: u8, face: u8, u: f64, v: f64, dir: DVec3, eps: f64, ryaw: f32, rs: f32) -> Option<Instance> {
        let r = self.radius;
        let rc = self.surface_radius(dir);
        let alt = (rc - r) as f32;
        if alt < sp.min_alt_m || alt > sp.max_alt_m {
            return None;
        }
        // Estencil en cruz a la distancia de la huella: da la normal (diferencias centrales de la SUPERFICIE ALTA,
        // igual que las normales del terreno) y el desnivel dentro de la huella.
        let pt = |du: f64, dv: f64| {
            let d = face_uv_to_dir(face, u + du, v + dv);
            d * self.surface_radius(d)
        };
        let (pu1, pu0, pv1, pv0) = (pt(eps, 0.0), pt(-eps, 0.0), pt(0.0, eps), pt(0.0, -eps));
        let mut n = cross(pu1 - pu0, pv1 - pv0).normalized();
        if n.dot(dir) < 0.0 {
            n = n * -1.0;
        }
        if n == DVec3::ZERO {
            return None;
        }
        let slope = n.dot(dir).clamp(-1.0, 1.0).acos().to_degrees() as f32;
        if slope > sp.max_slope_deg {
            return None;
        }
        // Planitud: centro, la cruz y un anillo de 12 puntos a la distancia de la huella y otro a media huella.
        let mut radii = vec![rc, pu1.length(), pu0.length(), pv1.length(), pv0.length()];
        for (k, f) in (0..12).map(|k| (k, 1.0)).chain((0..6).map(|k| (k * 2 + 1, 0.5))) {
            let a = k as f64 * std::f64::consts::TAU / 12.0;
            radii.push(self.surface_radius(face_uv_to_dir(face, u + eps * f * a.cos(), v + eps * f * a.sin())));
        }
        let (lo, hi) = radii.iter().fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
        if (hi - lo) as f32 > sp.max_range_m + 2.0 * sp.foot_m * slope.to_radians().tan() {
            // el desnivel permitido incluye el que ya explica la propia pendiente suave
            return None;
        }
        let scale = sp.scale.0 + (sp.scale.1 - sp.scale.0) * rs;
        let yaw = if sp.yaw_step > 0.0 {
            ((ryaw * 4.0).floor() * sp.yaw_step).rem_euclid(std::f32::consts::TAU)
        } else {
            ryaw * std::f32::consts::TAU
        };
        let up = n.to_f32();
        // Se entierra `sink_m` a lo largo de la normal para que la base no flote sobre la malla interpolada.
        let pos = dir * rc - n * sp.sink_m as f64;
        Some(Instance { species: si, u, v, dir, pos, up, yaw, scale })
    }
}
