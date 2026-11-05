//! LOD de esferas por tamaño en pantalla.
//!
//! # Fórmula
//! Para una arista de la malla de nivel `n` con ángulo central máximo `theta_n` (medido sobre la
//! malla real al construir), la silueta poligonal se hunde como máximo la sagita
//! `s_n = R * (1 - cos(theta_n / 2))` respecto del círculo ideal. A distancia `d` (al centro) con
//! campo de visión vertical `fov_y` y viewport de `H` píxeles, una unidad mide
//! `H / (2 d tan(fov_y/2))` píxeles, así que
//!
//! `error_px(n) = R * (1 - cos(theta_n/2)) * H / (2 d tan(fov_y/2))`.
//!
//! `select` devuelve el menor nivel con `error_px <= MAX_ERROR_PX` (2 px); si ninguno lo cumple
//! (o la cámara está dentro/pegada a la esfera) devuelve el nivel máximo. Como `error_px` crece al
//! acercarse, el nivel es monótono: más cerca implica nivel >=. Se ignora la amplitud del relieve
//! (la silueta deformada se muestrea en los mismos vértices).
use super::{Mesh, displaced_sphere};
use crate::math::Vec3;

/// Error de silueta tolerado en píxeles.
pub const MAX_ERROR_PX: f32 = 2.0;

pub struct SphereLods {
    /// `levels[n]` = icosfera con `n` subdivisiones, deformada con la misma altura.
    pub levels: Vec<Mesh>,
    /// Ángulo máximo (rad) subtendido por una arista en cada nivel.
    pub max_edge_angle: Vec<f32>,
}

impl SphereLods {
    /// Genera niveles `0..=max_subdiv` (ej. 5 => 20 a 20480 triángulos) con la misma `height`.
    pub fn new(max_subdiv: u32, height: &dyn Fn(Vec3) -> f32) -> Self {
        let mut levels = Vec::new();
        let mut max_edge_angle = Vec::new();
        for n in 0..=max_subdiv {
            let base = super::icosphere(n);
            let mut worst = 0.0f32;
            for t in base.indices.chunks_exact(3) {
                for i in 0..3 {
                    let a = base.vertices[t[i] as usize].position;
                    let b = base.vertices[t[(i + 1) % 3] as usize].position;
                    worst = worst.max(a.dot(b).clamp(-1.0, 1.0).acos());
                }
            }
            max_edge_angle.push(worst);
            levels.push(displaced_sphere(n, height));
        }
        Self { levels, max_edge_angle }
    }

    /// Error de silueta en píxeles del nivel `level`.
    pub fn error_px(&self, level: usize, distance: f32, radius: f32, fov_y: f32, viewport_h: f32) -> f32 {
        let scale = viewport_h / (2.0 * distance * (fov_y * 0.5).tan());
        let sag = radius * (1.0 - (self.max_edge_angle[level] * 0.5).cos());
        let e = sag * scale;
        if e.is_finite() { e } else { f32::INFINITY }
    }

    fn valid(&self, distance: f32, radius: f32, fov_y: f32, viewport_h: f32) -> bool {
        distance > radius.max(0.0)
            && distance.is_finite()
            && viewport_h > 0.0
            && fov_y > 0.0
            && fov_y < std::f32::consts::PI
    }

    /// Nivel mínimo cuyo error de silueta es <= `tol_px`.
    fn level_for(&self, tol_px: f32, d: f32, r: f32, fov_y: f32, h: f32) -> usize {
        let last = self.levels.len().saturating_sub(1);
        if !self.valid(d, r, fov_y, h) {
            return last;
        }
        (0..self.levels.len()).find(|&l| self.error_px(l, d, r, fov_y, h) <= tol_px).unwrap_or(last)
    }

    /// Elige el nivel por tamaño en pantalla (ver fórmula del módulo). Sin histéresis.
    pub fn select(&self, distance: f32, radius: f32, fov_y: f32, viewport_h: f32) -> usize {
        self.level_for(MAX_ERROR_PX, distance, radius, fov_y, viewport_h)
    }

    /// Como [`select`](Self::select) pero con histéresis para evitar parpadeo entre niveles:
    /// se sube de nivel solo si el actual supera `2px * (1 + margin)` y se baja solo cuando un nivel
    /// menor cumple `2px * (1 - margin)`. `margin` típico 0.25; `0` equivale a `select`.
    pub fn select_with_hysteresis(
        &self,
        current: usize,
        distance: f32,
        radius: f32,
        fov_y: f32,
        viewport_h: f32,
        margin: f32,
    ) -> usize {
        let last = self.levels.len().saturating_sub(1);
        let cur = current.min(last);
        let m = margin.clamp(0.0, 0.9);
        let ideal = self.select(distance, radius, fov_y, viewport_h);
        if ideal > cur {
            if self.valid(distance, radius, fov_y, viewport_h)
                && self.error_px(cur, distance, radius, fov_y, viewport_h) <= MAX_ERROR_PX * (1.0 + m)
            {
                cur
            } else {
                ideal
            }
        } else if ideal < cur {
            let lower = self.level_for(MAX_ERROR_PX * (1.0 - m), distance, radius, fov_y, viewport_h);
            lower.min(cur)
        } else {
            cur
        }
    }
}
