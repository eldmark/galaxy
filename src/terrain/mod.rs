//! Terreno cube-sphere con LOD continuo (quadtree por cara, chunks de 17x17, faldones, caché).
//!
//! Uso desde el coordinador (todo en el marco LOCAL del planeta, metros):
//! ```ignore
//! let mut terrain = Terrain::new(BodyKind::Verdalia, 3500.0, 8);          // una vez por planeta
//! let cam_local = rot_planeta.inverse() * (cam_mundo - centro_planeta);   // DVec3, rotación del planeta aplicada al revés
//! let frustum = Frustum::from_camera(cam_local, fwd_local, up_local, fov_y, aspect);   // opcional
//! for (key, mesh) in terrain.frame(cam_local, fov_y, viewport_h, Some(&frustum)) {
//!     // DrawCall { mesh: &*mesh, model: traslación(centro) * rotación_planeta  /* SIN escala */, .. }
//! }
//! let stats = &terrain.stats;   // TerrainStats para el HUD
//! ```
//! Las mallas están en metros, espacio local: `position / radius = dir * (1 + h)`, así que `p_local.normalized()`
//! es la dirección sobre la esfera para los shaders (que ya normalizan `p_local`).
//! Las alturas siempre salen de `relief::height(kind, dir)` salvo que se use [`Terrain::with_height`].
pub mod cache;
pub mod chunk;
pub mod cubesphere;
pub mod demo;
pub mod quadtree;

use crate::geometry::Mesh;
use crate::math::{DVec3, Vec3};
use crate::world::BodyKind;
pub use cache::ChunkCache;
pub use chunk::{ChunkKey, HeightFn, build_chunk};
pub use quadtree::{Frustum, Quadtree, TerrainStats};
use std::collections::HashSet;
use std::sync::Arc;

pub struct Terrain {
    pub kind: BodyKind,
    pub tree: Quadtree,
    height: Arc<HeightFn>,
    cache: ChunkCache,
    /// Máximo de chunks nuevos que se generan por cuadro (los demás se cubren con el ancestro ya listo).
    pub budget: usize,
    pub stats: TerrainStats,
}

impl Terrain {
    /// Terreno con el relieve oficial (`relief::height`). `max_level ≈ 8` da ~1 m entre vértices con radio 3500 m.
    pub fn new(kind: BodyKind, radius_m: f64, max_level: u8) -> Self {
        let amp = crate::relief::max_amplitude(kind) as f64;
        Self::with_height(kind, radius_m, max_level, amp, Arc::new(move |d| crate::relief::height(kind, d)))
    }

    /// Igual, con una función de altura propia (pruebas, demostración). `amp` es una cota de `|height|`.
    pub fn with_height(kind: BodyKind, radius_m: f64, max_level: u8, amp: f64, height: Arc<HeightFn>) -> Self {
        Self {
            kind,
            tree: Quadtree { radius: radius_m, amp, min_level: 2.min(max_level), max_level, px_threshold: 10.0, height: Some(height.clone()) },
            height,
            cache: ChunkCache::new(512),
            budget: 24,
            stats: TerrainStats::default(),
        }
    }

    pub fn radius(&self) -> f64 {
        self.tree.radius
    }
    pub fn set_px_threshold(&mut self, px: f64) {
        self.tree.px_threshold = px;
    }
    pub fn set_cache_capacity(&mut self, n: usize) {
        self.cache.capacity = n;
    }
    pub fn cache_len(&self) -> usize {
        self.cache.len()
    }

    /// Malla de un chunk (determinista, sin caché).
    pub fn build(&self, key: ChunkKey) -> Mesh {
        build_chunk(key, self.tree.radius, &*self.height)
    }

    /// Solo la selección del quadtree (sin generar nada). `frustum = None`: solo se descarta por horizonte.
    pub fn select(&self, cam_local: DVec3, fov_y: f64, viewport_h: f64, frustum: Option<&Frustum>) -> Vec<ChunkKey> {
        self.tree.select(cam_local, fov_y, viewport_h, frustum).0
    }

    /// Un cuadro: selecciona, genera hasta `budget` chunks faltantes en paralelo (los más cercanos primero) y devuelve
    /// los chunks a dibujar. Si una hoja no está lista se dibuja su ancestro más cercano ya listo (sin huecos ni solapes).
    pub fn frame(
        &mut self,
        cam_local: DVec3,
        fov_y: f64,
        viewport_h: f64,
        frustum: Option<&Frustum>,
    ) -> Vec<(ChunkKey, Arc<Mesh>)> {
        let t0 = std::time::Instant::now();
        let (leaves, mut stats) = self.tree.select(cam_local, fov_y, viewport_h, frustum);
        stats.ms_select = t0.elapsed().as_secs_f32() * 1000.0;
        let t1 = std::time::Instant::now();
        self.cache.begin_frame();
        let radius = self.tree.radius;
        let height = self.height.clone();
        let builder = move |k: ChunkKey| build_chunk(k, radius, &*height);

        // 1) faltantes, el más cercano primero, hasta el presupuesto
        let mut missing: Vec<(f64, ChunkKey)> = leaves
            .iter()
            .filter(|k| !self.cache.contains(k))
            .map(|k| ((self.tree.geom(k).center - cam_local).length(), *k))
            .collect();
        missing.sort_by(|a, b| a.0.total_cmp(&b.0));
        let now: Vec<ChunkKey> = missing.iter().take(self.budget).map(|m| m.1).collect();
        let mut generated = self.cache.generate(&now, &builder);

        // 2) lo que sigue sin estar listo se cubre con el ancestro más cercano cacheado; si no hay ninguno
        //    se fuerza el ancestro de nivel `min_level` (como mucho 6*4^min_level chunks, una sola vez).
        let min_level = self.tree.min_level;
        let ancestor_at_min = |k: ChunkKey| {
            let mut a = k;
            while a.level > min_level {
                a = a.parent().unwrap();
            }
            a
        };
        let mut forced = Vec::new();
        for (_, k) in missing.iter().skip(self.budget) {
            let mut a = *k;
            let mut found = false;
            while let Some(p) = a.parent() {
                a = p;
                if a.level < min_level {
                    break;
                }
                if self.cache.contains(&a) {
                    found = true;
                    break;
                }
            }
            if !found {
                forced.push(ancestor_at_min(*k));
            }
        }
        generated += self.cache.generate(&forced, &builder);

        // 3) clave a mostrar por hoja
        let mut shown: HashSet<ChunkKey> = HashSet::new();
        let mut fallback = 0u32;
        for k in &leaves {
            if self.cache.contains(k) {
                shown.insert(*k);
                continue;
            }
            fallback += 1;
            let mut a = *k;
            while let Some(p) = a.parent() {
                a = p;
                if self.cache.contains(&a) {
                    break;
                }
            }
            shown.insert(a);
        }
        // un ancestro sustituto cubre a sus descendientes ya listos: se quitan para no solapar
        let all: Vec<ChunkKey> = shown.iter().copied().collect();
        let mut out = Vec::with_capacity(all.len());
        for k in all {
            let mut covered = false;
            let mut a = k;
            while let Some(p) = a.parent() {
                a = p;
                if shown.contains(&a) {
                    covered = true;
                    break;
                }
            }
            if !covered {
                if let Some(m) = self.cache.get(&k) {
                    out.push((k, m));
                }
            }
        }
        out.sort_by_key(|(k, _)| *k);
        self.cache.evict();
        stats.ms_generate = t1.elapsed().as_secs_f32() * 1000.0;
        stats.generated = generated as u32;
        stats.fallback = fallback;
        stats.drawn = out.len() as u32;
        stats.triangles = out.iter().map(|(_, m)| m.triangle_count() as u32).sum();
        stats.cache_size = self.cache.len() as u32;
        self.stats = stats;
        out
    }

    /// Dirección -> radio de la superficie (m), para altura de la cámara sobre el suelo y colisión.
    pub fn surface_radius(&self, dir: Vec3) -> f64 {
        let d = dir.normalized();
        self.tree.radius * (1.0 + (self.height)(d) as f64)
    }
}

#[cfg(test)]
mod tests;
