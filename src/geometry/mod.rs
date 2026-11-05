//! Mallas y generación procedural de geometría. Dueño: agente `geometry`.
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
    /// Recalcula `bounds_center` y `bounds_radius` a partir de los vértices.
    pub fn update_bounds(&mut self) {
        todo!("geometry")
    }
}

/// Carga un OBJ (v, vt, vn, f con polígonos triangulados en abanico).
pub fn load_obj(_path: &str) -> Result<Mesh, Box<dyn std::error::Error>> {
    todo!("geometry")
}

/// Esfera unidad por subdivisión de un icosaedro (20 * 4^n triángulos), vértices en |p| = 1.
pub fn icosphere(_subdivisions: u32) -> Mesh {
    todo!("geometry")
}

/// Desplaza cada vértice de una esfera unidad: p' = p * (1 + height(p)) y recalcula normales.
pub fn displace(_mesh: &mut Mesh, _height: &dyn Fn(Vec3) -> f32) {
    todo!("geometry")
}
