//! Caché de chunks por clave: generación bajo demanda en paralelo (rayon) y expulsión por menos reciente.
use super::chunk::ChunkKey;
use crate::geometry::Mesh;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

struct Entry {
    mesh: Arc<Mesh>,
    last_used: u64,
}

pub struct ChunkCache {
    map: HashMap<ChunkKey, Entry>,
    /// Máximo de chunks guardados (~512). Los usados en el cuadro actual nunca se expulsan.
    pub capacity: usize,
    frame: u64,
}

impl ChunkCache {
    pub fn new(capacity: usize) -> Self {
        Self { map: HashMap::new(), capacity, frame: 0 }
    }
    pub fn len(&self) -> usize {
        self.map.len()
    }
    pub fn contains(&self, key: &ChunkKey) -> bool {
        self.map.contains_key(key)
    }
    /// Avanza el contador de cuadros (llamar una vez por cuadro, antes de pedir chunks).
    pub fn begin_frame(&mut self) {
        self.frame += 1;
    }
    /// Devuelve el chunk si está listo y lo marca como usado en este cuadro.
    pub fn get(&mut self, key: &ChunkKey) -> Option<Arc<Mesh>> {
        let f = self.frame;
        self.map.get_mut(key).map(|e| {
            e.last_used = f;
            e.mesh.clone()
        })
    }
    /// Genera en paralelo los chunks indicados que aún no existen. Devuelve cuántos generó.
    pub fn generate(&mut self, keys: &[ChunkKey], builder: &(dyn Fn(ChunkKey) -> Mesh + Sync)) -> usize {
        let mut todo: Vec<ChunkKey> = keys.iter().copied().filter(|k| !self.map.contains_key(k)).collect();
        todo.sort();
        todo.dedup();
        let meshes: Vec<(ChunkKey, Mesh)> = todo.par_iter().map(|&k| (k, builder(k))).collect();
        let n = meshes.len();
        for (k, m) in meshes {
            self.map.insert(k, Entry { mesh: Arc::new(m), last_used: self.frame });
        }
        n
    }
    /// Expulsa los menos recientes hasta `capacity`, sin tocar los usados en este cuadro.
    pub fn evict(&mut self) {
        if self.map.len() <= self.capacity {
            return;
        }
        let mut old: Vec<(u64, ChunkKey)> =
            self.map.iter().filter(|(_, e)| e.last_used < self.frame).map(|(k, e)| (e.last_used, *k)).collect();
        old.sort();
        let excess = self.map.len() - self.capacity;
        for (_, k) in old.into_iter().take(excess) {
            self.map.remove(&k);
        }
    }
}
