//! Selección de chunks: quadtree por cara que se divide por error en píxeles.
//!
//! Todo se calcula en el marco LOCAL del planeta (origen en su centro, metros, `f64`).
//! Error en píxeles = `separación entre vértices del chunk / distancia * foco`, es decir cuánto mide en pantalla
//! una celda de 1/16 del chunk (no el chunk entero: así `UMBRAL ≈ 6 px` significa "celdas de 6 px o menos").
use super::chunk::{ChunkKey, GRID, SKIRT_FRAC};
use super::cubesphere::{face_uv_to_dir};
use crate::math::DVec3;
use super::chunk::HeightFn;
use std::sync::Arc;

/// Planos del frustum en el marco local del planeta: `(normal hacia adentro, d)`; un punto `p` está dentro si `n·p + d >= 0`.
#[derive(Clone, Debug)]
pub struct Frustum {
    pub planes: Vec<(DVec3, f64)>,
}

impl Frustum {
    /// Frustum de una cámara (sin plano lejano): `eye`, `forward` y `up` en el marco local, `fov_y` en radianes.
    pub fn from_camera(eye: DVec3, forward: DVec3, up: DVec3, fov_y: f64, aspect: f64) -> Self {
        let f = forward.normalized();
        let r = super::cubesphere::cross(f, up).normalized();
        let u = super::cubesphere::cross(r, f);
        let (th, tw) = ((fov_y * 0.5).tan(), (fov_y * 0.5).tan() * aspect);
        // normales de los planos laterales apuntando hacia adentro
        let mk = |n: DVec3| (n.normalized(), -n.normalized().dot(eye));
        Frustum {
            planes: vec![
                mk(f),                 // cercano
                mk(r + f * tw),        // izquierdo
                mk(r * -1.0 + f * tw), // derecho
                mk(u + f * th),        // inferior
                mk(u * -1.0 + f * th), // superior
            ],
        }
    }
    pub fn intersects_sphere(&self, c: DVec3, r: f64) -> bool {
        self.planes.iter().all(|(n, d)| n.dot(c) + d >= -r)
    }
}

/// Contadores del último cuadro (para el HUD).
#[derive(Clone, Debug, Default)]
pub struct TerrainStats {
    pub visited: u32,
    pub culled_horizon: u32,
    pub culled_frustum: u32,
    /// Hojas seleccionadas por el quadtree.
    pub leaves: u32,
    pub min_level: u8,
    pub max_level: u8,
    /// Chunks distintos que se dibujan (tras sustituir los que faltan por su ancestro).
    pub drawn: u32,
    pub triangles: u32,
    /// Hojas que no estaban listas y se cubrieron con un ancestro.
    pub fallback: u32,
    /// Chunks generados en este cuadro.
    pub generated: u32,
    pub cache_size: u32,
    pub ms_select: f32,
    pub ms_generate: f32,
}

/// Geometría conservadora de un nodo.
#[derive(Clone, Copy, Debug)]
pub struct NodeGeom {
    /// Centro sobre la esfera de radio nominal.
    pub center: DVec3,
    pub cdir: DVec3,
    /// Radio de la esfera envolvente (incluye relieve y faldón).
    pub bound_r: f64,
    /// Radio geométrico (sin la incertidumbre del relieve): se usa para la distancia del error en píxeles.
    pub geo_r: f64,
    /// Radio angular (rad) visto desde el centro del planeta.
    pub ang: f64,
    /// Separación entre vértices (m).
    pub spacing: f64,
}

#[derive(Clone)]
pub struct Quadtree {
    pub radius: f64,
    /// Amplitud relativa máxima del relieve (cota de `|height|`).
    pub amp: f64,
    pub min_level: u8,
    pub max_level: u8,
    /// Umbral de error en píxeles para dividir.
    pub px_threshold: f64,
    /// Si existe, los nodos profundos (nivel >= 5) acotan el relieve con una rejilla de 5x5 muestras de la altura
    /// (+25 % de la amplitud de margen) en vez de la amplitud completa: esferas envolventes más justas para el frustum.
    pub height: Option<Arc<HeightFn>>,
}

impl Quadtree {
    pub fn geom(&self, key: &ChunkKey) -> NodeGeom {
        let r = self.radius;
        let (u0, u1, v0, v1) = (key.u_at(0), key.u_at(GRID), key.v_at(0), key.v_at(GRID));
        let (um, vm) = ((u0 + u1) * 0.5, (v0 + v1) * 0.5);
        let cdir = face_uv_to_dir(key.face, um, vm);
        let mut center = cdir * r;
        let corners = [(u0, v0), (u1, v0), (u1, v1), (u0, v1)].map(|(u, v)| face_uv_to_dir(key.face, u, v));
        let mids = [(um, v0), (u1, vm), (um, v1), (u0, vm)].map(|(u, v)| face_uv_to_dir(key.face, u, v));
        let mut ang = 0.0f64;
        let mut far = 0.0f64;
        for d in corners.iter().chain(mids.iter()) {
            ang = ang.max(cdir.dot(*d).clamp(-1.0, 1.0).acos());
            far = far.max(((*d - cdir) * r).length());
        }
        let mut relief = r * self.amp;
        if let (Some(h), true) = (&self.height, key.level >= 5 && self.amp > 0.0) {
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for a in 0..5 {
                for b in 0..5 {
                    let d = face_uv_to_dir(key.face, u0 + (u1 - u0) * a as f64 / 4.0, v0 + (v1 - v0) * b as f64 / 4.0);
                    let hv = h(d.to_f32()) as f64;
                    lo = lo.min(hv);
                    hi = hi.max(hv);
                }
            }
            // esfera centrada a la altura media de las muestras; margen = medio rango + 10 % de la amplitud
            // (heurística: detalle más fino que la rejilla de 5x5 queda cubierto por el margen)
            center = cdir * (r * (1.0 + 0.5 * (lo + hi)));
            relief = (r * (0.75 * (hi - lo) + 0.1 * self.amp)).min(r * self.amp);
        }
        let edge = (0..4).map(|i| ((corners[i] - corners[(i + 1) % 4]) * r).length()).fold(0.0f64, f64::max);
        let skirt = SKIRT_FRAC * edge;
        NodeGeom {
            center,
            cdir,
            bound_r: far + relief + skirt + 1e-6 * r,
            geo_r: far,
            ang,
            spacing: edge / GRID as f64,
        }
    }

    /// Ángulo máximo (desde la dirección de la cámara) en que puede verse un nodo, o `PI` si no se puede descartar.
    fn horizon_angle(&self, cam_len: f64) -> f64 {
        let lo = self.radius * (1.0 - self.amp);
        let hi = self.radius * (1.0 + self.amp);
        if cam_len <= lo * 1.000001 {
            return std::f64::consts::PI;
        }
        (lo / cam_len).acos() + if hi > lo { (lo / hi).acos() } else { 0.0 }
    }

    /// Hojas a dibujar. `frustum = None` desactiva el culling por vista (solo horizonte).
    pub fn select(
        &self,
        cam_local: DVec3,
        fov_y: f64,
        viewport_h: f64,
        frustum: Option<&Frustum>,
    ) -> (Vec<ChunkKey>, TerrainStats) {
        let focal = viewport_h / (2.0 * (fov_y * 0.5).tan());
        let cam_len = cam_local.length();
        let cam_dir = if cam_len > 1e-9 { cam_local * (1.0 / cam_len) } else { DVec3::ZERO };
        let hor = self.horizon_angle(cam_len);
        let mut stats = TerrainStats { min_level: u8::MAX, ..Default::default() };
        let mut out = Vec::new();
        let mut stack: Vec<ChunkKey> = (0..6u8).rev().map(ChunkKey::root).collect();
        while let Some(key) = stack.pop() {
            stats.visited += 1;
            let g = self.geom(&key);
            if cam_len > 0.0 {
                let a = cam_dir.dot(g.cdir).clamp(-1.0, 1.0).acos();
                if a - g.ang > hor {
                    stats.culled_horizon += 1;
                    continue;
                }
            }
            if let Some(f) = frustum {
                if !f.intersects_sphere(g.center, g.bound_r) {
                    stats.culled_frustum += 1;
                    continue;
                }
            }
            let dist = ((cam_local - g.center).length() - g.geo_r).max(0.01);
            let err_px = g.spacing / dist * focal;
            let split = key.level < self.max_level && (key.level < self.min_level || err_px > self.px_threshold);
            if split {
                let ch = key.children();
                for c in ch.iter().rev() {
                    stack.push(*c);
                }
            } else {
                stats.leaves += 1;
                stats.min_level = stats.min_level.min(key.level);
                stats.max_level = stats.max_level.max(key.level);
                out.push(key);
            }
        }
        if out.is_empty() {
            stats.min_level = 0;
        }
        (out, stats)
    }
}
