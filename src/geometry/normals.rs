//! Normales suaves ponderadas por área, soldadas por posición.
use super::Mesh;
use crate::math::Vec3;
use std::collections::HashMap;

fn key(p: Vec3) -> [u32; 3] {
    // `+ 0.0` convierte -0.0 en 0.0 para que ambos ceros coincidan.
    [(p.x + 0.0).to_bits(), (p.y + 0.0).to_bits(), (p.z + 0.0).to_bits()]
}

/// Normal suave por vértice: suma de normales de cara ponderadas por área, compartida entre
/// vértices con la misma posición (así las costuras de uv/duplicados no producen grietas de sombreado).
/// Devuelve ZERO para vértices sin triángulos válidos.
pub(crate) fn smooth_normals(mesh: &Mesh) -> Vec<Vec3> {
    let mut group_of = Vec::with_capacity(mesh.vertices.len());
    let mut map: HashMap<[u32; 3], usize> = HashMap::new();
    for v in &mesh.vertices {
        let n = map.len();
        group_of.push(*map.entry(key(v.position)).or_insert(n));
    }
    let mut acc = vec![Vec3::ZERO; map.len()];
    for t in mesh.indices.chunks_exact(3) {
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let (pa, pb, pc) = (mesh.vertices[a].position, mesh.vertices[b].position, mesh.vertices[c].position);
        let n = (pb - pa).cross(pc - pa);
        if !(n.x.is_finite() && n.y.is_finite() && n.z.is_finite()) {
            continue;
        }
        for i in [a, b, c] {
            acc[group_of[i]] += n;
        }
    }
    group_of.iter().map(|&g| acc[g].normalized()).collect()
}

/// Recalcula las normales de toda la malla como normales suaves. Vértices sin normal válida
/// conservan la que tenían.
pub fn recompute_normals(mesh: &mut Mesh) {
    let ns = smooth_normals(mesh);
    for (v, n) in mesh.vertices.iter_mut().zip(ns) {
        if n != Vec3::ZERO {
            v.normal = n;
        }
    }
}
