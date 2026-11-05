//! Formas simples: anillo plano, cubo y quad (útiles para pruebas de otros agentes).
use super::{Mesh, Vertex};
use crate::math::Vec3;
use std::f32::consts::PI;

fn vtx(p: Vec3, n: Vec3, uv: [f32; 2]) -> Vertex {
    Vertex { position: p, normal: n, uv, color: [1.0, 1.0, 1.0] }
}

/// Anillo plano en el plano XZ (y = 0), **doble cara**: cara superior (normal +Y, antihoraria vista
/// desde +Y) y cara inferior (normal -Y, orden inverso), como vértices y triángulos separados.
/// `uv.x` = radio normalizado (0 en `inner`, 1 en `outer`); `uv.y` = ángulo / 2pi (0..1, con la
/// costura duplicada: segments + 1 muestras por borde). `segments` mínimo 3.
pub fn ring_mesh(inner: f32, outer: f32, segments: u32) -> Mesh {
    let inner = inner.max(0.0);
    let outer = if outer > inner { outer } else { inner + 1e-3 };
    let s = segments.max(3);
    let mut m = Mesh::default();
    for (face, ny) in [(0u32, 1.0f32), (1, -1.0)] {
        let n = Vec3::new(0.0, ny, 0.0);
        let base = m.vertices.len() as u32;
        for i in 0..=s {
            let t = i as f32 / s as f32;
            let (sn, cs) = (2.0 * PI * t).sin_cos();
            m.vertices.push(vtx(Vec3::new(cs * inner, 0.0, sn * inner), n, [0.0, t]));
            m.vertices.push(vtx(Vec3::new(cs * outer, 0.0, sn * outer), n, [1.0, t]));
        }
        for i in 0..s {
            let (i0, o0, i1, o1) = (base + 2 * i, base + 2 * i + 1, base + 2 * i + 2, base + 2 * i + 3);
            if face == 0 {
                m.indices.extend_from_slice(&[i0, i1, o0, o0, i1, o1]);
            } else {
                m.indices.extend_from_slice(&[i0, o0, i1, o0, o1, i1]);
            }
        }
    }
    m.update_bounds();
    m
}

/// Quad unidad centrado en el origen en el plano XY, mirando a +Z (uv 0..1, v hacia arriba).
pub fn quad_mesh() -> Mesh {
    let n = Vec3::Z;
    let mut m = Mesh {
        vertices: vec![
            vtx(Vec3::new(-0.5, -0.5, 0.0), n, [0.0, 0.0]),
            vtx(Vec3::new(0.5, -0.5, 0.0), n, [1.0, 0.0]),
            vtx(Vec3::new(0.5, 0.5, 0.0), n, [1.0, 1.0]),
            vtx(Vec3::new(-0.5, 0.5, 0.0), n, [0.0, 1.0]),
        ],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..Default::default()
    };
    m.update_bounds();
    m
}

/// Cubo de lado 1 centrado en el origen, 24 vértices con normales planas por cara, uv 0..1 por cara.
pub fn cube_mesh() -> Mesh {
    let mut m = Mesh::default();
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Y, Vec3::Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::Z, Vec3::X),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::X, Vec3::Y),
    ];
    // Para cada cara: normal n y dos ejes del plano; se ordenan para que a x b = n.
    for (n, a, b) in faces {
        let (a, b) = if a.cross(b).dot(n) > 0.0 { (a, b) } else { (b, a) };
        let base = m.vertices.len() as u32;
        for (u, v) in [(0.0f32, 0.0f32), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
            let p = n * 0.5 + a * (u - 0.5) + b * (v - 0.5);
            m.vertices.push(vtx(p, n, [u, v]));
        }
        m.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    m.update_bounds();
    m
}
