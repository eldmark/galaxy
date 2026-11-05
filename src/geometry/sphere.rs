//! Icosfera (subdivisión procedural) y deformación por altura.
//!
//! # Etapa de subdivisión
//! [`icosphere`] **genera geometría nueva antes de rasterizar**: parte de un icosaedro (20 triángulos)
//! y cada nivel parte cada triángulo en 4 usando los puntos medios de sus aristas, proyectados a la
//! esfera unidad. Resultado: `20 * 4^n` triángulos y `10 * 4^n + 2` posiciones únicas.
//! # Etapa de deformación
//! [`displace`] mueve cada vértice `p' = normalize(p) * (1 + height(dir))` y recalcula normales.
//!
//! # Convenciones
//! - Polos del icosaedro en +Y y -Y (vértices exactos), anillos a latitud +-atan(1/2).
//! - Orden de caras antihorario visto desde fuera; normal = posición.
//! - uv esférico: `u = atan2(z, x) / 2pi` (0 en el meridiano +X, crece hacia +Z), `v = acos(y) / pi`
//!   (0 en el polo +Y, 1 en -Y). Los triángulos que cruzan la costura (meridiano +X) duplican sus
//!   vértices con `u + 1`, por lo que `u` puede llegar a ~1.1: el muestreador de texturas debe usar
//!   repetición (wrap) en u. Los vértices polares se duplican por triángulo con `u` = media de los
//!   otros dos, para que no haya remolino en el polo.
use super::{Mesh, Vertex, normals};
use crate::math::Vec3;
use std::collections::HashMap;
use std::f32::consts::PI;

/// Topología base: posiciones únicas y triángulos (antihorarios vistos desde fuera).
pub(crate) fn icosphere_topology(subdivisions: u32) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    let mut pos = Vec::new();
    pos.push(Vec3::new(0.0, 1.0, 0.0)); // 0: polo norte
    let ring_y = 1.0 / 5.0f32.sqrt();
    let ring_r = 2.0 / 5.0f32.sqrt();
    for k in 0..5 {
        let a = 2.0 * PI * k as f32 / 5.0; // anillo A: 1..=5
        pos.push(Vec3::new(ring_r * a.cos(), ring_y, ring_r * a.sin()));
    }
    for k in 0..5 {
        let a = 2.0 * PI * (k as f32 + 0.5) / 5.0; // anillo B: 6..=10
        pos.push(Vec3::new(ring_r * a.cos(), -ring_y, ring_r * a.sin()));
    }
    pos.push(Vec3::new(0.0, -1.0, 0.0)); // 11: polo sur
    let a = |k: u32| 1 + k % 5;
    let b = |k: u32| 6 + k % 5;
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for k in 0..5 {
        tris.push([0, a(k), a(k + 1)]);
        tris.push([a(k), b(k), a(k + 1)]);
        tris.push([a(k + 1), b(k), b(k + 1)]);
        tris.push([11, b(k + 1), b(k)]);
    }
    // Garantiza el orden antihorario visto desde fuera.
    for t in tris.iter_mut() {
        let (p0, p1, p2) = (pos[t[0] as usize], pos[t[1] as usize], pos[t[2] as usize]);
        if (p1 - p0).cross(p2 - p0).dot(p0 + p1 + p2) < 0.0 {
            t.swap(1, 2);
        }
    }
    for _ in 0..subdivisions {
        let mut mid: HashMap<(u32, u32), u32> = HashMap::new();
        let mut next = Vec::with_capacity(tris.len() * 4);
        let mut midpoint = |pos: &mut Vec<Vec3>, i: u32, j: u32| -> u32 {
            let key = (i.min(j), i.max(j));
            *mid.entry(key).or_insert_with(|| {
                let m = (pos[i as usize] + pos[j as usize]).normalized();
                pos.push(m);
                (pos.len() - 1) as u32
            })
        };
        for &[v0, v1, v2] in &tris {
            let m01 = midpoint(&mut pos, v0, v1);
            let m12 = midpoint(&mut pos, v1, v2);
            let m20 = midpoint(&mut pos, v2, v0);
            next.push([v0, m01, m20]);
            next.push([v1, m12, m01]);
            next.push([v2, m20, m12]);
            next.push([m01, m12, m20]);
        }
        tris = next;
    }
    (pos, tris)
}

fn lon_u(p: Vec3) -> f32 {
    let u = p.z.atan2(p.x) / (2.0 * PI);
    if u < 0.0 { u + 1.0 } else { u + 0.0 }
}

/// Esfera unidad por subdivisión de un icosaedro (20 * 4^n triángulos), vértices en |p| = 1.
/// Ver la documentación del módulo para uv y orden de caras. `n` se limita a 8.
pub fn icosphere(subdivisions: u32) -> Mesh {
    let n = subdivisions.min(8);
    let (pos, tris) = icosphere_topology(n);
    let mut mesh = Mesh::default();
    let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
    for t in &tris {
        let mut us = [0.0f32; 3];
        let mut pole = [false; 3];
        for i in 0..3 {
            let p = pos[t[i] as usize];
            pole[i] = p.x == 0.0 && p.z == 0.0;
            us[i] = lon_u(p);
        }
        // Costura: si los u de la cara (sin polos) abarcan más de media vuelta, sube los pequeños.
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 0..3 {
            if !pole[i] {
                lo = lo.min(us[i]);
                hi = hi.max(us[i]);
            }
        }
        if hi - lo > 0.5 {
            for i in 0..3 {
                if !pole[i] && us[i] < 0.5 {
                    us[i] += 1.0;
                }
            }
        }
        // Polo: u = media de los otros dos vértices.
        for i in 0..3 {
            if pole[i] {
                us[i] = (us[(i + 1) % 3] + us[(i + 2) % 3]) * 0.5;
            }
        }
        for i in 0..3 {
            let key = (t[i], us[i].to_bits());
            let idx = *cache.entry(key).or_insert_with(|| {
                let p = pos[t[i] as usize];
                mesh.vertices.push(Vertex {
                    position: p,
                    normal: p,
                    uv: [us[i], p.y.clamp(-1.0, 1.0).acos() / PI],
                    color: [1.0, 1.0, 1.0],
                });
                (mesh.vertices.len() - 1) as u32
            });
            mesh.indices.push(idx);
        }
    }
    mesh.update_bounds();
    mesh
}

/// Desplaza cada vértice de una esfera: `p' = normalize(p) * (1 + height(normalize(p)))`, recalcula
/// normales suaves (soldadas por posición) y los límites. Es la etapa de DEFORMACIÓN por ruido.
/// `height` debe ser pura/determinista: vértices duplicados (costura) reciben la misma altura.
pub fn displace(mesh: &mut Mesh, height: &dyn Fn(Vec3) -> f32) {
    for v in mesh.vertices.iter_mut() {
        let dir = v.position.normalized();
        if dir == Vec3::ZERO {
            continue;
        }
        let h = height(dir);
        let s = if h.is_finite() { (1.0 + h).max(1e-4) } else { 1.0 };
        v.position = dir * s;
    }
    normals::recompute_normals(mesh);
    mesh.update_bounds();
}

/// Icosfera subdividida y deformada con `height` (subdivisión + deformación).
pub fn displaced_sphere(subdiv: u32, height: &dyn Fn(Vec3) -> f32) -> Mesh {
    let mut m = icosphere(subdiv);
    displace(&mut m, height);
    m
}
