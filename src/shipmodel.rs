//! Modelo de la nave con el tren de aterrizaje como partes articuladas.
//!
//! Convención de `Ship.gear` (ver `ship/physics.rs::update_gear`): **1 = desplegado** (aterrizado
//! o en descenso bajo 250 m), **0 = plegado** (vuelo). El OBJ está modelado desplegado (0°) y la
//! pata plegada gira -90° alrededor de +X sobre su pivote.
use crate::geometry::{Mesh, load_obj_groups};
use crate::math::{Mat4, Vec3};
use crate::raster::{DrawCall, FragmentShader, Uniforms};

/// Nombres de los grupos de tren y sus pivotes (coordenadas de nave, de `assets/models/ship.md`).
const GEAR_DEFS: [(&str, [f32; 3]); 3] = [
    ("gear_left", [-2.0, -1.0, 2.0]),
    ("gear_right", [2.0, -1.0, 2.0]),
    ("gear_front", [0.0, -1.0, -3.2]),
];
/// Ángulo con el tren plegado (rad): -90°.
pub const FOLDED_ANGLE: f32 = -std::f32::consts::FRAC_PI_2;

pub struct GearPart {
    pub mesh: Mesh,
    pub pivot: Vec3,
    pub axis: Vec3,
}

pub struct ShipModel {
    /// Todo lo que no es tren (casco, cabina, toberas).
    pub body: Mesh,
    /// Izquierda, derecha, frontal.
    pub gears: [GearPart; 3],
}

fn merge(meshes: Vec<Mesh>) -> Mesh {
    let mut out = Mesh::default();
    for m in meshes {
        let off = out.vertices.len() as u32;
        out.vertices.extend(m.vertices);
        out.indices.extend(m.indices.iter().map(|i| i + off));
    }
    out.update_bounds();
    out
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl ShipModel {
    pub fn load(path: &str) -> Result<ShipModel, Box<dyn std::error::Error>> {
        Ok(Self::from_groups(load_obj_groups(path)?)?)
    }

    pub fn from_groups(groups: Vec<(String, Mesh)>) -> Result<ShipModel, String> {
        let mut body = Vec::new();
        let mut found: [Option<Mesh>; 3] = [None, None, None];
        for (name, mesh) in groups {
            match GEAR_DEFS.iter().position(|(n, _)| *n == name) {
                Some(i) => found[i] = Some(mesh),
                None => body.push(mesh),
            }
        }
        if body.is_empty() {
            return Err("el modelo no tiene cuerpo".into());
        }
        let mut part = |i: usize| -> Result<GearPart, String> {
            let mesh = found[i]
                .take()
                .ok_or_else(|| format!("falta el grupo '{}' en el modelo de la nave", GEAR_DEFS[i].0))?;
            let p = GEAR_DEFS[i].1;
            Ok(GearPart { mesh, pivot: Vec3::new(p[0], p[1], p[2]), axis: Vec3::new(1.0, 0.0, 0.0) })
        };
        let gears = [part(0)?, part(1)?, part(2)?];
        Ok(ShipModel { body: merge(body), gears })
    }

    /// Matriz modelo de cada pata (izq, der, frontal): `ship_model * T(p) * Rx(ang) * T(-p)`.
    /// `gear`: 1 desplegado, 0 plegado; la curva es suave (ease in-out).
    pub fn gear_matrices(&self, gear: f32, ship_model: Mat4) -> [Mat4; 3] {
        let gear = if gear.is_finite() { gear } else { 1.0 };
        let angle = FOLDED_ANGLE * (1.0 - ease_in_out(gear));
        let rot = Mat4::rotation_x(angle);
        let mut out = [ship_model; 3];
        for (o, g) in out.iter_mut().zip(self.gears.iter()) {
            *o = ship_model * Mat4::translation(g.pivot) * rot * Mat4::translation(g.pivot * -1.0);
        }
        out
    }

    /// Un call de cuerpo y hasta tres de tren (se omite el tren si está del todo plegado).
    pub fn draw_calls<'a>(
        &'a self,
        model: Mat4,
        gear: f32,
        shader: &'a dyn FragmentShader,
        uniforms: Uniforms,
    ) -> Vec<DrawCall<'a>> {
        let mut calls = vec![DrawCall { mesh: &self.body, model, shader, uniforms: Uniforms { model, ..uniforms } }];
        if gear > 0.0 {
            let mats = self.gear_matrices(gear, model);
            for (g, m) in self.gears.iter().zip(mats) {
                calls.push(DrawCall { mesh: &g.mesh, model: m, shader, uniforms: Uniforms { model: m, ..uniforms } });
            }
        }
        calls
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::load_obj;

    const PATH: &str = "assets/models/ship.obj";

    #[test]
    fn grupos_suman_los_mismos_triangulos() {
        let groups = load_obj_groups(PATH).unwrap();
        let sum: usize = groups.iter().map(|(_, m)| m.triangle_count()).sum();
        assert_eq!(sum, load_obj(PATH).unwrap().triangle_count());
        for (_, m) in &groups {
            assert!(m.bounds_radius > 0.0);
            assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
        }
    }

    #[test]
    fn existen_los_tres_grupos_de_tren() {
        let names: Vec<String> = load_obj_groups(PATH).unwrap().into_iter().map(|(n, _)| n).collect();
        for n in ["hull", "cockpit", "nozzle", "gear_left", "gear_right", "gear_front"] {
            assert!(names.iter().any(|x| x == n), "falta {n}");
        }
        let sm = ShipModel::load(PATH).unwrap();
        assert!(sm.body.triangle_count() > 0);
    }

    fn lowest_y(g: &GearPart, m: &Mat4) -> f32 {
        g.mesh.vertices.iter().map(|v| m.transform_point(v.position).y).fold(f32::MAX, f32::min)
    }

    #[test]
    fn plegado_sube_la_pata_y_el_pivote_es_fijo() {
        let sm = ShipModel::load(PATH).unwrap();
        let dep = sm.gear_matrices(1.0, Mat4::IDENTITY);
        let fold = sm.gear_matrices(0.0, Mat4::IDENTITY);
        for i in 0..3 {
            assert!(lowest_y(&sm.gears[i], &fold[i]) > lowest_y(&sm.gears[i], &dep[i]) + 1.0);
            for g in [0.0, 0.3, 0.5, 1.0] {
                let m = sm.gear_matrices(g, Mat4::IDENTITY)[i];
                let p = sm.gears[i].pivot;
                assert!((m.transform_point(p) - p).length() < 1e-4);
            }
        }
        let v = sm.gears[0].mesh.vertices[0].position;
        assert!((dep[0].transform_point(v) - v).length() < 1e-4);
    }

    #[test]
    fn sin_nan_y_cantidad_de_calls() {
        let sm = ShipModel::load(PATH).unwrap();
        for g in [-1.0, 0.0, 0.25, 0.5, 1.0, 2.0, f32::NAN] {
            for m in sm.gear_matrices(g, Mat4::IDENTITY) {
                assert!(m.m.iter().flatten().all(|x| x.is_finite()));
            }
        }
    }
}
