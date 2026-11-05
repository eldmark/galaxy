//! Chunk: malla de 17x17 vértices de un nodo (cara, nivel, x, y) del quadtree, con faldones.
//!
//! - Posición (espacio LOCAL del planeta, metros): `dir * radius * (1 + height(dir))`. Dividida entre el radio
//!   da `dir * (1 + h)`, así que `p_local.normalized()` es la dirección sobre la esfera para los shaders.
//! - Normal: producto cruz de diferencias centrales de la SUPERFICIE ALTA (no de la malla) en coordenadas
//!   `(u, v)` globales, evaluadas con la misma fórmula en cada chunk: los vértices compartidos entre chunks del mismo
//!   nivel salen idénticos y no hay costuras de sombreado.
//! - Faldones: copia del borde bajada `SKIRT_FRAC` (2 %) del tamaño del chunk hacia el centro del planeta,
//!   para tapar las grietas entre chunks de distinto nivel. Triángulos orientados hacia afuera del chunk.
//! - Índices: primero los `GRID*GRID*6` de la rejilla (sirve para medir área), luego los de los faldones.
use super::cubesphere::{cross, face_uv_to_dir};
use crate::geometry::{Mesh, Vertex};
use crate::math::{DVec3, Vec3};

/// Celdas por lado de un chunk (17 vértices).
pub const GRID: usize = 16;
pub const VERTS: usize = GRID + 1;
/// Profundidad del faldón como fracción del tamaño (largo) del chunk.
pub const SKIRT_FRAC: f64 = 0.02;
/// Cantidad de índices de la rejilla principal (el resto son faldones).
pub const GRID_INDICES: usize = GRID * GRID * 6;
pub const GRID_VERTS: usize = VERTS * VERTS;

/// Altura relativa de la superficie: `radio_real = radius * (1 + h(dir))`.
pub type HeightFn = dyn Fn(Vec3) -> f32 + Send + Sync;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ChunkKey {
    pub face: u8,
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl ChunkKey {
    pub fn root(face: u8) -> Self {
        Self { face, level: 0, x: 0, y: 0 }
    }
    /// Nodos por lado en este nivel.
    pub fn nodes_per_side(&self) -> u32 {
        1u32 << self.level
    }
    pub fn parent(&self) -> Option<ChunkKey> {
        (self.level > 0).then(|| ChunkKey { face: self.face, level: self.level - 1, x: self.x >> 1, y: self.y >> 1 })
    }
    pub fn children(&self) -> [ChunkKey; 4] {
        let (l, x, y, f) = (self.level + 1, self.x * 2, self.y * 2, self.face);
        [
            ChunkKey { face: f, level: l, x, y },
            ChunkKey { face: f, level: l, x: x + 1, y },
            ChunkKey { face: f, level: l, x, y: y + 1 },
            ChunkKey { face: f, level: l, x: x + 1, y: y + 1 },
        ]
    }
    /// `true` si `self` es ancestro estricto de `other`.
    pub fn is_ancestor_of(&self, other: &ChunkKey) -> bool {
        if self.face != other.face || self.level >= other.level {
            return false;
        }
        let s = other.level - self.level;
        (other.x >> s) == self.x && (other.y >> s) == self.y
    }
    /// Coordenada `u` del vértice de columna `i` (0..=16). Aritmética entera: vértices compartidos son idénticos bit a bit.
    pub fn u_at(&self, i: usize) -> f64 {
        ((self.x as u64 * GRID as u64 + i as u64) as f64) / ((self.nodes_per_side() as u64 * GRID as u64) as f64 * 0.5) - 1.0
    }
    pub fn v_at(&self, j: usize) -> f64 {
        ((self.y as u64 * GRID as u64 + j as u64) as f64) / ((self.nodes_per_side() as u64 * GRID as u64) as f64 * 0.5) - 1.0
    }
    /// Paso entre vértices en coordenadas `(u, v)`.
    pub fn uv_step(&self) -> f64 {
        2.0 / (self.nodes_per_side() as f64 * GRID as f64)
    }
}

/// Punto de la superficie (espacio local, metros) para `(cara, u, v)`.
fn surface_point(face: u8, u: f64, v: f64, radius: f64, height: &HeightFn) -> (DVec3, DVec3) {
    let dir = face_uv_to_dir(face, u, v);
    let h = height(dir.to_f32()) as f64;
    (dir, dir * (radius * (1.0 + h)))
}

fn sub(a: DVec3, b: DVec3) -> DVec3 {
    a - b
}

/// Normal exacta de la superficie en `(u, v)` por diferencias centrales de la altura.
pub fn surface_normal(face: u8, u: f64, v: f64, eps: f64, radius: f64, height: &HeightFn) -> DVec3 {
    let pu = sub(surface_point(face, u + eps, v, radius, height).1, surface_point(face, u - eps, v, radius, height).1);
    let pv = sub(surface_point(face, u, v + eps, radius, height).1, surface_point(face, u, v - eps, radius, height).1);
    let n = cross(pu, pv).normalized();
    if n == DVec3::ZERO { face_uv_to_dir(face, u, v) } else { n }
}

/// Largo aproximado (m) del lado del chunk sobre la esfera de radio `radius`.
pub fn chunk_size_m(key: &ChunkKey, radius: f64) -> f64 {
    radius * std::f64::consts::FRAC_PI_2 / key.nodes_per_side() as f64
}

/// Genera la malla de un chunk. Determinista y sin estado: la misma clave da la misma malla.
pub fn build_chunk(key: ChunkKey, radius: f64, height: &HeightFn) -> Mesh {
    let eps = key.uv_step() * 0.5;
    let depth = SKIRT_FRAC * chunk_size_m(&key, radius);
    let uv_to_01 = |a: f64| (a * 0.5 + 0.5) as f32;
    let mut vertices = Vec::with_capacity(GRID_VERTS + 4 * VERTS);
    for j in 0..VERTS {
        for i in 0..VERTS {
            let (u, v) = (key.u_at(i), key.v_at(j));
            let (_, p) = surface_point(key.face, u, v, radius, height);
            let n = surface_normal(key.face, u, v, eps, radius, height);
            vertices.push(Vertex {
                position: p.to_f32(),
                normal: n.to_f32(),
                uv: [uv_to_01(u), uv_to_01(v)],
                color: [1.0, 1.0, 1.0],
            });
        }
    }
    let idx = |i: usize, j: usize| (j * VERTS + i) as u32;
    let mut indices = Vec::with_capacity(GRID_INDICES + 4 * GRID * 6);
    for j in 0..GRID {
        for i in 0..GRID {
            let (a, b, c, d) = (idx(i, j), idx(i + 1, j), idx(i + 1, j + 1), idx(i, j + 1));
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    // Faldones. Cada borde recorre 17 vértices "a lo largo" y se duplican bajados hacia el centro.
    // (vértice del borde, índice del faldón) por borde; `flip` invierte el orden para apuntar hacia afuera.
    let edges: [(Box<dyn Fn(usize) -> (usize, usize)>, bool); 4] = [
        (Box::new(|k| (k, 0)), false),
        (Box::new(|k| (k, GRID)), true),
        (Box::new(|k| (0, k)), true),
        (Box::new(|k| (GRID, k)), false),
    ];
    for (at, flip) in edges.iter() {
        let base = vertices.len() as u32;
        for k in 0..VERTS {
            let (i, j) = at(k);
            let mut v = vertices[j * VERTS + i];
            let p = v.position.to_f64();
            let dir = p.normalized();
            v.position = (p - dir * depth).to_f32();
            vertices.push(v);
        }
        for k in 0..GRID {
            let (a, b) = (idx_of(at(k)), idx_of(at(k + 1)));
            let (a2, b2) = (base + k as u32, base + k as u32 + 1);
            if *flip {
                indices.extend_from_slice(&[a, b, b2, a, b2, a2]);
            } else {
                indices.extend_from_slice(&[a, b2, b, a, a2, b2]);
            }
        }
    }
    let mut mesh = Mesh { vertices, indices, bounds_center: Vec3::ZERO, bounds_radius: 0.0 };
    mesh.update_bounds();
    mesh
}

fn idx_of((i, j): (usize, usize)) -> u32 {
    (j * VERTS + i) as u32
}

/// Área (m²) de la rejilla principal de un chunk (sin faldones).
pub fn grid_area(mesh: &Mesh) -> f64 {
    let mut a = 0.0;
    for t in mesh.indices[..GRID_INDICES].chunks_exact(3) {
        let p = |i: u32| mesh.vertices[i as usize].position.to_f64();
        let (p0, p1, p2) = (p(t[0]), p(t[1]), p(t[2]));
        a += 0.5 * cross(p1 - p0, p2 - p0).length();
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(_: Vec3) -> f32 {
        0.0
    }
    fn bumpy(d: Vec3) -> f32 {
        0.02 * ((d.x * 9.0).sin() * (d.y * 7.0).cos() + (d.z * 11.0 + d.x * 3.0).sin()) * 0.5
    }

    #[test]
    fn counts_and_finite() {
        let m = build_chunk(ChunkKey { face: 2, level: 3, x: 5, y: 2 }, 3500.0, &bumpy);
        assert_eq!(m.vertices.len(), 289 + 68);
        assert_eq!(m.triangle_count(), 512 + 128);
        for v in &m.vertices {
            for c in [v.position.x, v.position.y, v.position.z, v.normal.x, v.normal.y, v.normal.z, v.uv[0], v.uv[1]] {
                assert!(c.is_finite());
            }
            assert!((v.normal.length() - 1.0).abs() < 1e-3);
        }
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        assert!(m.bounds_radius > 0.0);
    }

    #[test]
    fn deterministic() {
        let k = ChunkKey { face: 4, level: 5, x: 7, y: 20 };
        let a = build_chunk(k, 3500.0, &bumpy);
        let b = build_chunk(k, 3500.0, &bumpy);
        assert_eq!(a.indices, b.indices);
        for (p, q) in a.vertices.iter().zip(&b.vertices) {
            assert_eq!(p.position, q.position);
            assert_eq!(p.normal, q.normal);
        }
    }

    #[test]
    fn winding_points_outward_and_p_local_is_direction_times_height() {
        for face in 0..6 {
            let m = build_chunk(ChunkKey { face, level: 2, x: 1, y: 2 }, 3500.0, &flat);
            for t in m.indices[..GRID_INDICES].chunks_exact(3) {
                let p = |i: u32| m.vertices[i as usize].position;
                let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
                assert!(n.dot(p(t[0])) > 0.0, "cara {face}: triángulo hacia adentro");
            }
            for v in &m.vertices[..GRID_VERTS] {
                assert!((v.position.length() - 3500.0).abs() < 0.01);
            }
        }
    }

    #[test]
    fn skirts_face_outward_and_hang_below() {
        let k = ChunkKey { face: 0, level: 3, x: 3, y: 4 };
        let m = build_chunk(k, 3500.0, &flat);
        let c = m.bounds_center;
        let depth = (SKIRT_FRAC * chunk_size_m(&k, 3500.0)) as f32;
        for v in &m.vertices[GRID_VERTS..] {
            let drop = 3500.0 - v.position.length();
            assert!((drop - depth).abs() < 0.01 * depth + 1e-3);
        }
        for t in m.indices[GRID_INDICES..].chunks_exact(3) {
            let p = |i: u32| m.vertices[i as usize].position;
            let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
            let mid = (p(t[0]) + p(t[1]) + p(t[2])) * (1.0 / 3.0);
            // el borde sale del centro del chunk: la normal del faldón debe alejarse de él
            assert!(n.dot(mid - c) > 0.0, "faldón apunta hacia adentro");
        }
    }

    #[test]
    fn same_level_neighbours_share_edge_positions_and_normals() {
        let a = build_chunk(ChunkKey { face: 1, level: 4, x: 5, y: 6 }, 3500.0, &bumpy);
        let b = build_chunk(ChunkKey { face: 1, level: 4, x: 6, y: 6 }, 3500.0, &bumpy);
        for j in 0..VERTS {
            let (va, vb) = (a.vertices[j * VERTS + GRID], b.vertices[j * VERTS]);
            assert_eq!(va.position, vb.position);
            assert_eq!(va.normal, vb.normal);
        }
    }

    #[test]
    fn flat_area_matches_sphere() {
        let tot: f64 = (0..6)
            .map(|f| grid_area(&build_chunk(ChunkKey::root(f), 1000.0, &flat)))
            .sum();
        let exact = 4.0 * std::f64::consts::PI * 1e6;
        assert!((tot - exact).abs() / exact < 0.02, "{tot} vs {exact}");
    }
}
