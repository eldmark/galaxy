//! Mapeo cube-sphere: (cara, u, v) <-> dirección unitaria. Todo en `f64`.
//!
//! Cada cara del cubo es un cuadrado con `u, v` en `[-1, 1]`. El punto del cubo se lleva a la esfera con
//! la corrección `x' = x * sqrt(1 - y²/2 - z²/2 + y²z²/3)` (y análogas), que da celdas casi iguales
//! y un resultado EXACTAMENTE unitario. Caras: 0 = +X, 1 = -X, 2 = +Y, 3 = -Y, 4 = +Z, 5 = -Z.
//! En todas, `tu x tv = normal`, así que la malla `(i,j) -> (i+1,j) -> (i+1,j+1)` es antihoraria hacia afuera.
use crate::math::DVec3;

pub const FACES: usize = 6;

pub fn cross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}

/// (normal, tu, tv) de la cara.
pub fn face_basis(face: u8) -> (DVec3, DVec3, DVec3) {
    let d = DVec3::new;
    match face {
        0 => (d(1., 0., 0.), d(0., 0., -1.), d(0., 1., 0.)),
        1 => (d(-1., 0., 0.), d(0., 0., 1.), d(0., 1., 0.)),
        2 => (d(0., 1., 0.), d(1., 0., 0.), d(0., 0., -1.)),
        3 => (d(0., -1., 0.), d(1., 0., 0.), d(0., 0., 1.)),
        4 => (d(0., 0., 1.), d(1., 0., 0.), d(0., 1., 0.)),
        _ => (d(0., 0., -1.), d(-1., 0., 0.), d(0., 1., 0.)),
    }
}

/// Punto del cubo `[-1,1]³` -> esfera unitaria (corrección de esquinas).
pub fn cube_to_sphere(p: DVec3) -> DVec3 {
    let (x2, y2, z2) = (p.x * p.x, p.y * p.y, p.z * p.z);
    DVec3::new(
        p.x * (1.0 - y2 * 0.5 - z2 * 0.5 + y2 * z2 / 3.0).max(0.0).sqrt(),
        p.y * (1.0 - z2 * 0.5 - x2 * 0.5 + z2 * x2 / 3.0).max(0.0).sqrt(),
        p.z * (1.0 - x2 * 0.5 - y2 * 0.5 + x2 * y2 / 3.0).max(0.0).sqrt(),
    )
}

/// `(cara, u, v)` -> dirección unitaria.
pub fn face_uv_to_dir(face: u8, u: f64, v: f64) -> DVec3 {
    let (n, tu, tv) = face_basis(face);
    cube_to_sphere(n + tu * u + tv * v)
}

/// Cara dominante de una dirección (la de mayor componente absoluta).
pub fn dir_face(d: DVec3) -> u8 {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    if ax >= ay && ax >= az {
        if d.x >= 0.0 { 0 } else { 1 }
    } else if ay >= az {
        if d.y >= 0.0 { 2 } else { 3 }
    } else if d.z >= 0.0 {
        4
    } else {
        5
    }
}

/// Inversa sobre una cara dada (Gauss-Newton sobre el mapeo directo; parte de la proyección gnomónica).
pub fn dir_to_uv_on_face(face: u8, dir: DVec3) -> (f64, f64) {
    let d = dir.normalized();
    let (n, tu, tv) = face_basis(face);
    let k = d.dot(n).max(1e-9);
    let (mut u, mut v) = (d.dot(tu) / k, d.dot(tv) / k);
    let h = 1e-6;
    for _ in 0..12 {
        let f = face_uv_to_dir(face, u, v);
        let r = d - f;
        let fu = (face_uv_to_dir(face, u + h, v) - face_uv_to_dir(face, u - h, v)) * (0.5 / h);
        let fv = (face_uv_to_dir(face, u, v + h) - face_uv_to_dir(face, u, v - h)) * (0.5 / h);
        let (a, b, c) = (fu.dot(fu), fu.dot(fv), fv.dot(fv));
        let (ru, rv) = (fu.dot(r), fv.dot(r));
        let det = a * c - b * b;
        if det.abs() < 1e-18 {
            break;
        }
        let (du, dv) = ((c * ru - b * rv) / det, (a * rv - b * ru) / det);
        u += du;
        v += dv;
        if du.abs() + dv.abs() < 1e-14 {
            break;
        }
    }
    (u, v)
}

/// Dirección -> `(cara, u, v)` con la cara dominante.
pub fn dir_to_face_uv(dir: DVec3) -> (u8, f64, f64) {
    let face = dir_face(dir);
    let (u, v) = dir_to_uv_on_face(face, dir);
    (face, u, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basis_is_right_handed_and_unit() {
        for f in 0..FACES as u8 {
            let (n, tu, tv) = face_basis(f);
            let c = cross(tu, tv);
            assert!((c - n).length() < 1e-12, "cara {f}");
            assert!((face_uv_to_dir(f, 0.0, 0.0) - n).length() < 1e-12);
        }
    }

    #[test]
    fn mapping_is_unit_length() {
        for f in 0..FACES as u8 {
            for i in 0..=20 {
                for j in 0..=20 {
                    let d = face_uv_to_dir(f, i as f64 / 10.0 - 1.0, j as f64 / 10.0 - 1.0);
                    assert!((d.length() - 1.0).abs() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn round_trip() {
        for f in 0..FACES as u8 {
            for i in 0..=16 {
                for j in 0..=16 {
                    let (u, v) = (i as f64 / 8.0 - 1.0, j as f64 / 8.0 - 1.0);
                    let d = face_uv_to_dir(f, u, v);
                    let (f2, u2, v2) = dir_to_face_uv(d);
                    let d2 = face_uv_to_dir(f2, u2, v2);
                    assert!((d - d2).length() < 1e-9, "cara {f} ({u},{v})");
                    let (u3, v3) = dir_to_uv_on_face(f, d);
                    assert!((u3 - u).abs() < 1e-8 && (v3 - v).abs() < 1e-8, "cara {f} ({u},{v}) -> ({u3},{v3})");
                }
            }
        }
    }

    /// Continuidad en aristas: un punto en el borde de una cara cae en el borde de otra cara
    /// (coordenada +-1 allí) y un paso pequeño hacia adentro mueve la dirección poco.
    #[test]
    fn edge_continuity_between_neighbour_faces() {
        for f in 0..FACES as u8 {
            for s in 0..=16 {
                let t = s as f64 / 8.0 - 1.0;
                for (u, v) in [(1.0, t), (-1.0, t), (t, 1.0), (t, -1.0)] {
                    let d = face_uv_to_dir(f, u, v);
                    let mut found = false;
                    for g in 0..FACES as u8 {
                        if g == f {
                            continue;
                        }
                        let (a, b) = dir_to_uv_on_face(g, d);
                        let on_edge = ((a.abs() - 1.0).abs() < 1e-7 && b.abs() <= 1.0 + 1e-7)
                            || ((b.abs() - 1.0).abs() < 1e-7 && a.abs() <= 1.0 + 1e-7);
                        if on_edge {
                            let back = face_uv_to_dir(g, a, b);
                            assert!((back - d).length() < 1e-9);
                            found = true;
                        }
                    }
                    assert!(found, "cara {f} ({u},{v}) sin vecina");
                    let e = 1e-4;
                    let d2 = face_uv_to_dir(f, u * (1.0 - e), v * (1.0 - e));
                    assert!((d - d2).length() < 5e-4);
                }
            }
        }
    }
}
