//! Mallas de los modelos: versión completa (OBJ propio de `assets/models/`) y versión simplificada de LOD
//! generada en código (tronco + conos, prismas, cajas), con normales planas y color por vértice.
use super::species::ModelId;
use crate::geometry::{Mesh, Vertex, load_obj};
use crate::math::Vec3;
use std::sync::Arc;

/// Constructor de mallas facetadas (cada triángulo con sus 3 vértices y su normal plana).
#[derive(Default)]
struct Facets {
    mesh: Mesh,
}

impl Facets {
    /// Triángulo orientado hacia afuera: `inside` es un punto interior de la pieza (si la normal apunta hacia él, se invierte).
    fn tri(&mut self, a: Vec3, b: Vec3, c: Vec3, col: [f32; 3], inside: Vec3) {
        let mut n = (b - a).cross(c - a);
        let (mut b, mut c) = (b, c);
        if n.dot((a + b + c) * (1.0 / 3.0) - inside) < 0.0 {
            std::mem::swap(&mut b, &mut c);
            n = n * -1.0;
        }
        let n = n.normalized();
        let base = self.mesh.vertices.len() as u32;
        for p in [a, b, c] {
            self.mesh.vertices.push(Vertex { position: p, normal: n, uv: [0.0, 0.0], color: col });
        }
        self.mesh.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
    fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, col: [f32; 3], inside: Vec3) {
        self.tri(a, b, c, col, inside);
        self.tri(a, c, d, col, inside);
    }
    /// Tronco de cono / prisma de `n` lados entre dos anillos horizontales (sin tapas). `r1 = 0` da un cono.
    fn frustum(&mut self, y0: f32, r0: f32, y1: f32, r1: f32, n: usize, rot: f32, col: [f32; 3], cap_bottom: bool) {
        let ring = |y: f32, r: f32, i: usize| {
            let a = rot + std::f32::consts::TAU * i as f32 / n as f32;
            Vec3::new(r * a.cos(), y, r * a.sin())
        };
        let inside = Vec3::new(0.0, (y0 + y1) * 0.5, 0.0);
        for i in 0..n {
            let j = (i + 1) % n;
            let (a, b, c, d) = (ring(y0, r0, i), ring(y0, r0, j), ring(y1, r1, j), ring(y1, r1, i));
            if r1 <= 1e-6 {
                self.tri(a, b, Vec3::new(0.0, y1, 0.0), col, inside);
            } else {
                self.quad(a, b, c, d, col, inside);
            }
            if cap_bottom {
                self.tri(Vec3::new(0.0, y0, 0.0), a, b, col, inside);
            }
        }
    }
    /// Caja alineada a los ejes sin la cara inferior (nunca se ve desde arriba).
    fn boxed(&mut self, x0: f32, x1: f32, y0: f32, y1: f32, z0: f32, z1: f32, col: [f32; 3]) {
        let p = |x, y, z| Vec3::new(x, y, z);
        let ins = p((x0 + x1) * 0.5, (y0 + y1) * 0.5, (z0 + z1) * 0.5);
        self.quad(p(x0, y1, z0), p(x0, y1, z1), p(x1, y1, z1), p(x1, y1, z0), col, ins);
        self.quad(p(x0, y0, z1), p(x1, y0, z1), p(x1, y1, z1), p(x0, y1, z1), col, ins);
        self.quad(p(x1, y0, z0), p(x0, y0, z0), p(x0, y1, z0), p(x1, y1, z0), col, ins);
        self.quad(p(x1, y0, z1), p(x1, y0, z0), p(x1, y1, z0), p(x1, y1, z1), col, ins);
        self.quad(p(x0, y0, z0), p(x0, y0, z1), p(x0, y1, z1), p(x0, y1, z0), col, ins);
    }
    fn finish(mut self) -> Mesh {
        self.mesh.update_bounds();
        self.mesh
    }
}

/// Versión simplificada de cada modelo (~10 veces menos triángulos), con la misma silueta y colores.
pub fn low_poly(model: ModelId) -> Mesh {
    let mut f = Facets::default();
    match model {
        // tronco prisma de 4 lados + dos conos de 6 lados apilados (26 triángulos)
        ModelId::Tree => {
            f.frustum(0.0, 0.5, 2.6, 0.3, 4, 0.0, [0.30, 0.20, 0.12], false);
            f.frustum(2.0, 2.6, 6.4, 0.0, 6, 0.0, [0.16, 0.42, 0.16], true);
            f.frustum(5.0, 1.7, 9.6, 0.0, 6, 0.4, [0.26, 0.56, 0.21], false);
        }
        // una espira de 6 lados: tramo recto y punta (18 triángulos)
        ModelId::Obsidian => {
            f.frustum(0.0, 0.95, 1.6, 0.8, 6, 0.1, [0.07, 0.05, 0.11], false);
            f.frustum(1.6, 0.8, 5.0, 0.0, 6, 0.1, [0.09, 0.06, 0.14], false);
        }
        // un cristal: prisma de 6 lados y punta (18 triángulos)
        ModelId::Ice => {
            f.frustum(0.0, 0.8, 2.8, 0.85, 6, 0.0, [0.62, 0.86, 0.95], false);
            f.frustum(2.8, 0.85, 4.0, 0.0, 6, 0.0, [0.75, 0.92, 0.99], false);
        }
        // zócalo + cabeza + morro como tres cajas (30 triángulos)
        ModelId::Building => {
            f.boxed(-3.0, 3.0, 0.0, 1.5, -3.0, 3.0, [0.42, 0.44, 0.52]);
            f.boxed(-2.5, 2.5, 1.5, 6.0, -2.25, 2.25, [0.92, 0.90, 0.84]);
            f.boxed(-1.6, 1.6, 1.5, 3.3, 2.25, 3.4, [0.95, 0.60, 0.68]);
        }
    }
    f.finish()
}

/// Modelos completos y simplificados de los cuatro tipos.
pub struct Models {
    pub high: [Arc<Mesh>; 4],
    pub low: [Arc<Mesh>; 4],
}

impl Models {
    /// Carga los OBJ de `dir` (p. ej. `assets/models`) y genera las versiones simplificadas.
    pub fn load(dir: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut high = Vec::new();
        for m in ModelId::ALL {
            high.push(Arc::new(load_obj(&format!("{dir}/{}", m.file()))?));
        }
        Ok(Self::from_high(high))
    }
    pub fn from_high(high: Vec<Arc<Mesh>>) -> Self {
        let low = ModelId::ALL.map(|m| Arc::new(low_poly(m)));
        Self { high: high.try_into().ok().expect("cuatro modelos"), low }
    }
    pub fn mesh(&self, m: ModelId, high: bool) -> &Mesh {
        if high { &self.high[m.index()] } else { &self.low[m.index()] }
    }
}
