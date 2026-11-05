//! Mallas y generación procedural de geometría. Dueño: agente `geometry`.
//!
//! Convenciones: triángulos antihorarios vistos desde fuera (normal geométrica
//! `(b-a)x(c-a)` apunta hacia afuera), mano derecha, +Y arriba.
//!
//! Etapas procedurales (puntúan en la rúbrica):
//! - **Subdivisión**: [`icosphere`] genera geometría nueva (20 * 4^n triángulos) antes de rasterizar.
//! - **Deformación**: [`displace`] / [`displaced_sphere`] mueven cada vértice según una función de ruido.
//! - **LOD**: [`SphereLods`] elige el nivel de subdivisión según el tamaño en pantalla.
pub mod lod;
mod normals;
pub mod obj;
pub mod shapes;
pub mod sphere;

pub use lod::SphereLods;
pub use normals::recompute_normals;
pub use obj::{load_obj, parse_obj};
pub use shapes::{cube_mesh, quad_mesh, ring_mesh};
pub use sphere::{displace, displaced_sphere, icosphere};

use crate::math::Vec3;

#[derive(Clone, Copy, Debug, Default)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: [f32; 2],
    /// Color por vértice en 0..1 (blanco si el modelo no define color).
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    /// Triángulos: tres índices consecutivos, orden antihorario visto desde fuera.
    pub indices: Vec<u32>,
    /// Esfera envolvente en espacio del objeto (para frustum culling).
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
}

impl Mesh {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
    /// Recalcula `bounds_center` (centro de la caja) y `bounds_radius` (distancia máxima al centro).
    pub fn update_bounds(&mut self) {
        if self.vertices.is_empty() {
            self.bounds_center = Vec3::ZERO;
            self.bounds_radius = 0.0;
            return;
        }
        let first = self.vertices[0].position;
        let (mut lo, mut hi) = (first, first);
        for v in &self.vertices {
            let p = v.position;
            lo = Vec3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = Vec3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        let c = (lo + hi) * 0.5;
        let r2 = self.vertices.iter().map(|v| (v.position - c).length_sq()).fold(0.0f32, f32::max);
        self.bounds_center = c;
        self.bounds_radius = r2.sqrt();
    }
}

#[cfg(test)]
mod tests;
