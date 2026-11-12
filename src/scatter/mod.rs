//! Objetos en superficie: árboles, espiras, cristales y edificios repartidos por semilla sobre el terreno.
//!
//! Rúbrica que cubre este módulo:
//! - **Tipos de modelo propio sobre un planeta** (5 pts por tipo): `ModelId::{Tree, Obsidian, Ice, Building}`
//!   (OBJ hechos con `cargo run --bin generate_assets`), uno por planeta según `species::species_for`.
//! - **Distribución procedural reproducible** (10 pts): `place.rs`. Cada celda de scatter (un chunk de nivel 8) usa
//!   `rng = hash(semilla, cuerpo, cara, nivel, x, y)`, una rejilla con jitter y ruido fbm de valor para agrupar
//!   (bosques, racimos). La semilla global se fija con `--seed N` (`set_global_seed`, por defecto 1).
//!   Misma semilla = mismas posiciones sin importar el orden de generación; semilla distinta = otras.
//! - **Posición y orientación sobre superficie curva mientras el planeta rota** (5 pts): cada instancia vive en el
//!   espacio LOCAL del planeta: base en `dir * radio_de_la_superficie` y eje Y local = normal del relieve (diferencias
//!   centrales de `relief::height`), con giro aleatorio alrededor de la normal. Las instancias de un chunk se
//!   HORNEAN en una sola malla en ese mismo espacio local, así que se dibujan con el MISMO `model` que el chunk de
//!   terreno (traslación relativa al ojo * rotación del planeta): rotan y orbitan con el planeta sin hacer nada más.
//! - **Descarte / LOD de instancias con contadores**: por distancia al chunk, modelo completo (< `lod_dist[0]`),
//!   versión simplificada (< `lod_dist[1]`) o nada. Cambia los triángulos que recibe el rasterizador y se reporta
//!   en [`ScatterStats`]. Un chunk = un DrawCall (el rasterizador no paga una llamada por árbol).
//!
//! Uso desde `Scene::render` (marco local del planeta):
//! ```ignore
//! let objs = scatter.frame(&chunk_keys, cam_local);        // Vec<(ChunkKey, Arc<Mesh>)>
//! // DrawCall { mesh, model: <el mismo model del terreno>, shader: PropShader, .. }
//! ```
pub mod models;
pub mod place;
pub mod species;
#[cfg(test)]
mod tests;

use crate::geometry::{Mesh, Vertex};
use crate::math::{DVec3, Vec3};
use crate::terrain::chunk::{GRID, chunk_size_m};
use crate::terrain::cubesphere::face_uv_to_dir;
use crate::terrain::ChunkKey;
use crate::world::BodyKind;
pub use models::Models;
pub use place::{CELL_LEVEL, Instance, MAX_PER_CELL, Placer};
use rayon::prelude::*;
pub use species::{ModelId, Species, species_for};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Nivel mínimo de chunk que recibe objetos (`N_SCATTER`). Un chunk de nivel `l <= CELL_LEVEL` agrupa las
/// `4^(CELL_LEVEL - l)` celdas que contiene, así que las posiciones no cambian al cambiar el nivel del terreno
/// (no hay "saltos" de árboles al acercarse). Los chunks de nivel menor (lejanos o ancestros provisionales) no llevan objetos.
pub const N_SCATTER: u8 = 6;
/// Distancia máxima (múltiplo de `lod_dist[1]`) a la que aún se cuentan las instancias descartadas por distancia.
const COUNT_RANGE: f32 = 2.0;
/// Mallas horneadas nuevas por cuadro (el resto se hornea en los cuadros siguientes; mientras tanto no hay objetos).
const BAKE_BUDGET: usize = 4;
const MESH_CACHE: usize = 192;
const CELL_CACHE: usize = 4096;

static GLOBAL_SEED: AtomicU64 = AtomicU64::new(1);
/// Semilla global de la distribución (`--seed N`). Cámbiala ANTES de crear la `Scene`.
pub fn set_global_seed(seed: u64) {
    GLOBAL_SEED.store(seed, Ordering::Relaxed);
}
pub fn global_seed() -> u64 {
    GLOBAL_SEED.load(Ordering::Relaxed)
}

/// Nivel de detalle de las instancias de un chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lod {
    High,
    Low,
}

/// Contadores del último cuadro (se suman por planeta con `+=`).
#[derive(Clone, Debug, Default)]
pub struct ScatterStats {
    /// Chunks hoja del terreno con nivel >= `N_SCATTER`.
    pub chunks_considered: u32,
    /// Esos chunks que quedaron más lejos del rango de conteo (no se evalúan).
    pub chunks_out_of_range: u32,
    /// Chunks con malla de objetos dibujada (= DrawCalls de objetos).
    pub draw_calls: u32,
    /// Instancias en los chunks evaluados (dentro del rango de conteo).
    pub instances_total: u32,
    /// Instancias dibujadas con el modelo completo / con el simplificado.
    pub instances_high: u32,
    pub instances_low: u32,
    /// Instancias descartadas por estar más lejos que `lod_dist[1]`.
    pub instances_culled_distance: u32,
    /// Triángulos de objetos que recibe el rasterizador.
    pub triangles: u32,
    /// Triángulos que habría si todo lo dibujado usara el modelo completo (sin LOD).
    pub triangles_without_lod: u32,
    /// Mallas horneadas en este cuadro y tamaño de las cachés.
    pub baked: u32,
    pub cache_meshes: u32,
    pub cache_cells: u32,
}

impl ScatterStats {
    pub fn instances_shown(&self) -> u32 {
        self.instances_high + self.instances_low
    }
    pub fn add(&mut self, o: &ScatterStats) {
        self.chunks_considered += o.chunks_considered;
        self.chunks_out_of_range += o.chunks_out_of_range;
        self.draw_calls += o.draw_calls;
        self.instances_total += o.instances_total;
        self.instances_high += o.instances_high;
        self.instances_low += o.instances_low;
        self.instances_culled_distance += o.instances_culled_distance;
        self.triangles += o.triangles;
        self.triangles_without_lod += o.triangles_without_lod;
        self.baked += o.baked;
        self.cache_meshes += o.cache_meshes;
        self.cache_cells += o.cache_cells;
    }
}

struct Baked {
    mesh: Arc<Mesh>,
    instances: u32,
    tris: u32,
    tris_high: u32,
    last_used: u64,
}

/// Distribución y dibujo de objetos de UN cuerpo.
pub struct Scatter {
    pub placer: Placer,
    models: Arc<Models>,
    /// Si es `false`, todo lo que está dentro de `lod_dist[1]` usa el modelo completo (para medir el efecto del LOD).
    pub use_lod: bool,
    pub stats: ScatterStats,
    cells: HashMap<(ChunkKey, u64), (Arc<Vec<Instance>>, u64)>,
    meshes: HashMap<(ChunkKey, Lod, u64), Baked>,
    frame_no: u64,
    bake_budget: usize,
    lod_dist: [f32; 2],
    amp_m: f64,
}

impl Scatter {
    /// `None` si el cuerpo no tiene especies de objetos (sol, Siroco).
    pub fn new(kind: BodyKind, radius: f64, seed: u64, models: Arc<Models>) -> Option<Self> {
        let placer = Placer::new(kind, radius, seed);
        if placer.species.is_empty() {
            return None;
        }
        let lod_dist = [0, 1].map(|k| placer.species.iter().map(|s| s.lod_dist[k]).fold(0.0, f32::max));
        Some(Self {
            amp_m: radius * crate::relief::max_amplitude(kind) as f64,
            placer,
            models,
            use_lod: true,
            stats: ScatterStats::default(),
            cells: HashMap::new(),
            meshes: HashMap::new(),
            frame_no: 0,
            bake_budget: BAKE_BUDGET,
            lod_dist,
        })
    }

    pub fn seed(&self) -> u64 {
        self.placer.seed
    }
    pub fn lod_dist(&self) -> [f32; 2] {
        self.lod_dist
    }
    pub fn set_bake_budget(&mut self, n: usize) {
        self.bake_budget = n;
    }

    /// Celdas de nivel `CELL_LEVEL` que cubre un chunk (una sola celda ancestro si el chunk es más fino).
    pub fn cells_of(key: ChunkKey) -> Vec<ChunkKey> {
        if key.level >= CELL_LEVEL {
            let s = key.level - CELL_LEVEL;
            return vec![ChunkKey { face: key.face, level: CELL_LEVEL, x: key.x >> s, y: key.y >> s }];
        }
        let s = CELL_LEVEL - key.level;
        let n = 1u32 << s;
        let mut v = Vec::with_capacity((n * n) as usize);
        for dy in 0..n {
            for dx in 0..n {
                v.push(ChunkKey { face: key.face, level: CELL_LEVEL, x: (key.x << s) + dx, y: (key.y << s) + dy });
            }
        }
        v
    }

    fn in_chunk(key: ChunkKey, i: &Instance) -> bool {
        key.level <= CELL_LEVEL || {
            let (u0, u1, v0, v1) = (key.u_at(0), key.u_at(GRID), key.v_at(0), key.v_at(GRID));
            i.u >= u0 && i.u < u1 && i.v >= v0 && i.v < v1
        }
    }

    /// Instancias de un chunk (sin caché, para pruebas y herramientas). Determinista.
    pub fn instances(&self, key: ChunkKey) -> Vec<Instance> {
        let mut out = Vec::new();
        for c in Self::cells_of(key) {
            out.extend(self.placer.cell_instances(c).into_iter().filter(|i| Self::in_chunk(key, i)));
        }
        out
    }

    /// Distancia (m) de la cámara al punto más cercano del chunk (cota conservadora, 0 si está dentro).
    pub fn chunk_distance(&self, key: ChunkKey, cam_local: DVec3) -> f64 {
        let (uc, vc) = ((key.u_at(0) + key.u_at(GRID)) * 0.5, (key.v_at(0) + key.v_at(GRID)) * 0.5);
        let center = face_uv_to_dir(key.face, uc, vc) * self.placer.radius;
        let bound = chunk_size_m(&key, self.placer.radius) * 0.72 + self.amp_m;
        ((cam_local - center).length() - bound).max(0.0)
    }

    fn lod_for(&self, dist: f64) -> Option<Lod> {
        let [d0, d1] = self.lod_dist;
        if dist >= d1 as f64 {
            None
        } else if self.use_lod && dist >= d0 as f64 {
            Some(Lod::Low)
        } else {
            Some(Lod::High)
        }
    }

    /// Hornea las instancias de un chunk en UNA malla en el espacio local del planeta.
    /// Vértice final = `pos + R * (escala * v)`, con `R = [x, y = normal, z]` de la instancia.
    pub fn bake(&self, instances: &[Instance], lod: Lod) -> (Mesh, u32) {
        let high = lod == Lod::High;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut tris_high = 0u32;
        for inst in instances {
            let sp = &self.placer.species[inst.species as usize];
            let src = self.models.mesh(sp.model, high);
            tris_high += self.models.mesh(sp.model, true).triangle_count() as u32;
            let (x, y, z) = inst.basis();
            let base = vertices.len() as u32;
            for v in &src.vertices {
                let q = x * (v.position.x * inst.scale) + y * (v.position.y * inst.scale) + z * (v.position.z * inst.scale);
                let n = x * v.normal.x + y * v.normal.y + z * v.normal.z;
                vertices.push(Vertex { position: (inst.pos + q.to_f64()).to_f32(), normal: n, uv: v.uv, color: v.color });
            }
            indices.extend(src.indices.iter().map(|i| i + base));
        }
        let mut mesh = Mesh { vertices, indices, bounds_center: Vec3::ZERO, bounds_radius: 0.0 };
        mesh.update_bounds();
        (mesh, tris_high)
    }

    /// Un cuadro. `chunks`: las hojas que dibuja el terreno (en el marco local del planeta). Devuelve una malla
    /// horneada por chunk con objetos (mismo `model` que el terreno) y actualiza `self.stats`.
    pub fn frame(&mut self, chunks: &[ChunkKey], cam_local: DVec3) -> Vec<(ChunkKey, Arc<Mesh>)> {
        self.frame_no += 1;
        let frame = self.frame_no;
        let seed = self.placer.seed;
        let mut st = ScatterStats::default();
        let count_range = (self.lod_dist[1] * COUNT_RANGE) as f64;

        // 1) chunks candidatos y distancia
        let mut cand: Vec<(ChunkKey, f64)> = Vec::new();
        for &k in chunks {
            if k.level < N_SCATTER {
                continue;
            }
            st.chunks_considered += 1;
            let d = self.chunk_distance(k, cam_local);
            if d > count_range {
                st.chunks_out_of_range += 1;
            } else {
                cand.push((k, d));
            }
        }

        // 2) celdas que faltan (generación en paralelo)
        let mut missing: Vec<ChunkKey> = Vec::new();
        for (k, _) in &cand {
            for c in Self::cells_of(*k) {
                if !self.cells.contains_key(&(c, seed)) {
                    missing.push(c);
                }
            }
        }
        missing.sort();
        missing.dedup();
        let placer = &self.placer;
        let made: Vec<(ChunkKey, Vec<Instance>)> = missing.par_iter().map(|c| (*c, placer.cell_instances(*c))).collect();
        for (c, v) in made {
            self.cells.insert((c, seed), (Arc::new(v), frame));
        }

        // 3) cuenta, LOD y qué mallas hay que hornear
        struct Plan {
            key: ChunkKey,
            lod: Lod,
            dist: f64,
        }
        let mut plans: Vec<Plan> = Vec::new();
        for (k, d) in &cand {
            let mut count = 0u32;
            for c in Self::cells_of(*k) {
                if let Some(e) = self.cells.get_mut(&(c, seed)) {
                    e.1 = frame;
                    count += if k.level <= CELL_LEVEL { e.0.len() as u32 } else { e.0.iter().filter(|i| Self::in_chunk(*k, i)).count() as u32 };
                }
            }
            st.instances_total += count;
            match self.lod_for(*d) {
                None => st.instances_culled_distance += count,
                Some(lod) if count > 0 => plans.push(Plan { key: *k, lod, dist: *d }),
                Some(_) => {}
            }
        }

        // 4) hornear (los más cercanos primero, con presupuesto)
        let mut to_bake: Vec<&Plan> = plans.iter().filter(|p| !self.meshes.contains_key(&(p.key, p.lod, seed))).collect();
        to_bake.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        to_bake.truncate(self.bake_budget);
        let work: Vec<(ChunkKey, Lod, Vec<Instance>)> = to_bake.iter().map(|p| (p.key, p.lod, self.instances_cached(p.key))).collect();
        let this = &*self;
        let baked: Vec<(ChunkKey, Lod, Mesh, u32, u32)> = work
            .par_iter()
            .map(|(k, lod, inst)| {
                let (m, th) = this.bake(inst, *lod);
                (*k, *lod, m, inst.len() as u32, th)
            })
            .collect();
        st.baked = baked.len() as u32;
        for (k, lod, m, n, th) in baked {
            let tris = m.triangle_count() as u32;
            self.meshes.insert((k, lod, seed), Baked { mesh: Arc::new(m), instances: n, tris, tris_high: th, last_used: frame });
        }

        // 5) salida: la malla del LOD pedido o, si aún no está, la del otro LOD ya horneada
        let mut out = Vec::new();
        for p in &plans {
            let other = if p.lod == Lod::High { Lod::Low } else { Lod::High };
            let used = [p.lod, other].into_iter().find(|l| self.meshes.contains_key(&(p.key, *l, seed)));
            let Some(l) = used else { continue };
            let b = self.meshes.get_mut(&(p.key, l, seed)).unwrap();
            b.last_used = frame;
            match l {
                Lod::High => st.instances_high += b.instances,
                Lod::Low => st.instances_low += b.instances,
            }
            st.triangles += b.tris;
            st.triangles_without_lod += b.tris_high;
            st.draw_calls += 1;
            out.push((p.key, b.mesh.clone()));
        }
        out.sort_by_key(|(k, _)| *k);
        self.evict(frame);
        st.cache_meshes = self.meshes.len() as u32;
        st.cache_cells = self.cells.len() as u32;
        self.stats = st;
        out
    }

    fn instances_cached(&self, key: ChunkKey) -> Vec<Instance> {
        let seed = self.placer.seed;
        let mut out = Vec::new();
        for c in Self::cells_of(key) {
            if let Some((v, _)) = self.cells.get(&(c, seed)) {
                out.extend(v.iter().copied().filter(|i| Self::in_chunk(key, i)));
            }
        }
        out
    }

    /// Expulsa lo menos reciente cuando las cachés pasan su límite (lo usado en este cuadro nunca se expulsa).
    fn evict(&mut self, frame: u64) {
        if self.meshes.len() > MESH_CACHE {
            let mut v: Vec<((ChunkKey, Lod, u64), u64)> = self.meshes.iter().filter(|(_, b)| b.last_used < frame).map(|(k, b)| (*k, b.last_used)).collect();
            v.sort_by_key(|x| x.1);
            let extra = self.meshes.len() - MESH_CACHE;
            for (k, _) in v.into_iter().take(extra) {
                self.meshes.remove(&k);
            }
        }
        if self.cells.len() > CELL_CACHE {
            let mut v: Vec<((ChunkKey, u64), u64)> = self.cells.iter().filter(|(_, e)| e.1 < frame).map(|(k, e)| (*k, e.1)).collect();
            v.sort_by_key(|x| x.1);
            let extra = self.cells.len() - CELL_CACHE;
            for (k, _) in v.into_iter().take(extra) {
                self.cells.remove(&k);
            }
        }
    }

    /// Centro de la celda de nivel `CELL_LEVEL` con más instancias (sumando sus 4 vecinas) entre `samples` direcciones
    /// repartidas por la esfera. Sirve para llevar la cámara de las capturas a un bosque, un racimo de cristales, etc.
    pub fn find_dense_spot(&self, samples: usize) -> DVec3 {
        self.find_dense_spot_facing(samples, None)
    }

    /// Igual, pero solo direcciones con `dir . facing > 0.6` (p. ej. el lado iluminado por el sol, en marco local).
    pub fn find_dense_spot_facing(&self, samples: usize, facing: Option<DVec3>) -> DVec3 {
        use crate::terrain::cubesphere::dir_to_face_uv;
        let n = 1u32 << CELL_LEVEL;
        let count = |c: ChunkKey| self.placer.cell_instances(c).len();
        let mut best = (0usize, DVec3::new(0.0, 1.0, 0.0));
        let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
        for i in 0..samples {
            let y = 1.0 - 2.0 * (i as f64 + 0.5) / samples as f64;
            let r = (1.0 - y * y).sqrt();
            let d = DVec3::new(r * (golden * i as f64).cos(), y, r * (golden * i as f64).sin());
            if facing.is_some_and(|f| d.dot(f) < 0.6) {
                continue;
            }
            let (face, u, v) = dir_to_face_uv(d);
            let (cx, cy) = ((((u * 0.5 + 0.5) * n as f64) as u32).min(n - 1), (((v * 0.5 + 0.5) * n as f64) as u32).min(n - 1));
            let mut score = 0;
            for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if x >= 0 && y >= 0 && x < n as i32 && y < n as i32 {
                    score += count(ChunkKey { face, level: CELL_LEVEL, x: x as u32, y: y as u32 });
                }
            }
            if score > best.0 {
                let cell = ChunkKey { face, level: CELL_LEVEL, x: cx, y: cy };
                let (uc, vc) = ((cell.u_at(0) + cell.u_at(GRID)) * 0.5, (cell.v_at(0) + cell.v_at(GRID)) * 0.5);
                best = (score, face_uv_to_dir(face, uc, vc));
            }
        }
        best.1
    }
}
