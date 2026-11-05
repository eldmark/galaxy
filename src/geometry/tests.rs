use super::*;
use crate::math::Vec3;
use std::collections::{HashMap, HashSet};

const SCRATCH: &str = "/tmp/claude-1000/-home-dmark123-Documents-homework-graficas-galaxy/146d9493-3f05-4f8b-853c-4441b3071a9a/scratchpad";

fn key(p: Vec3) -> [u32; 3] {
    [(p.x + 0.0).to_bits(), (p.y + 0.0).to_bits(), (p.z + 0.0).to_bits()]
}

fn tri(m: &Mesh, i: usize) -> [Vec3; 3] {
    let t = &m.indices[i * 3..i * 3 + 3];
    [m.vertices[t[0] as usize].position, m.vertices[t[1] as usize].position, m.vertices[t[2] as usize].position]
}

fn unique_positions(m: &Mesh) -> usize {
    m.vertices.iter().map(|v| key(v.position)).collect::<HashSet<_>>().len()
}

#[test]
fn icosphere_counts() {
    for n in 0..=4u32 {
        let m = icosphere(n);
        let f = 4usize.pow(n);
        assert_eq!(m.triangle_count(), 20 * f);
        assert_eq!(unique_positions(&m), 10 * f + 2, "n={n}");
        assert!(m.vertices.len() >= 10 * f + 2);
        assert!(m.indices.iter().all(|&i| (i as usize) < m.vertices.len()));
    }
    assert_eq!(icosphere(0).vertices.len() >= 12, true);
}

#[test]
fn icosphere_unit_nondegenerate_outward() {
    for n in 0..=4u32 {
        let m = icosphere(n);
        for v in &m.vertices {
            assert!((v.position.length() - 1.0).abs() < 1e-5);
            assert!((v.normal - v.position).length() < 1e-5);
        }
        for i in 0..m.triangle_count() {
            let [a, b, c] = tri(&m, i);
            let nrm = (b - a).cross(c - a);
            assert!(nrm.length() > 1e-6, "degenerado n={n} tri={i}");
            assert!(nrm.dot(a + b + c) > 0.0, "cara hacia adentro n={n} tri={i}");
        }
        assert!((m.bounds_radius - 1.0).abs() < 0.1);
    }
}

#[test]
fn icosphere_is_closed_manifold() {
    for n in 0..=3u32 {
        let m = icosphere(n);
        let mut edges: HashMap<([u32; 3], [u32; 3]), u32> = HashMap::new();
        for i in 0..m.triangle_count() {
            let p = tri(&m, i);
            for e in 0..3 {
                let (a, b) = (key(p[e]), key(p[(e + 1) % 3]));
                *edges.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        assert!(edges.values().all(|&c| c == 2), "n={n}");
        // Euler: V - E + F = 2
        assert_eq!(unique_positions(&m) as i64 - edges.len() as i64 + m.triangle_count() as i64, 2);
    }
}

#[test]
fn icosphere_uv_reasonable() {
    let m = icosphere(3);
    for v in &m.vertices {
        assert!(v.uv[0] >= -1e-6 && v.uv[0] <= 1.2, "u={}", v.uv[0]);
        assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
    }
    // Ningún triángulo cruza la costura con saltos de u: la extensión en u es pequeña.
    for t in m.indices.chunks_exact(3) {
        let us: Vec<f32> = t.iter().map(|&i| m.vertices[i as usize].uv[0]).collect();
        let span = us.iter().cloned().fold(f32::MIN, f32::max) - us.iter().cloned().fold(f32::MAX, f32::min);
        assert!(span < 0.3, "span={span}");
    }
}

#[test]
fn displace_constant_scales() {
    let mut m = icosphere(2);
    displace(&mut m, &|_| 0.5);
    for v in &m.vertices {
        assert!((v.position.length() - 1.5).abs() < 1e-4);
        assert!(v.normal.dot(v.position.normalized()) > 0.98);
    }
    assert!((m.bounds_radius - 1.5).abs() < 0.15);
}

#[test]
fn displace_normals_outward_and_seam_consistent() {
    let m = displaced_sphere(4, &|d| 0.1 * (3.0 * d.x).sin() * (2.0 * d.y + d.z).cos());
    for v in &m.vertices {
        assert!(v.normal.dot(v.position.normalized()) > 0.5);
        assert!((v.normal.length() - 1.0).abs() < 1e-3);
    }
    let mut by_pos: HashMap<[u32; 3], Vec3> = HashMap::new();
    for v in &m.vertices {
        let n = *by_pos.entry(key(v.position)).or_insert(v.normal);
        assert!((n - v.normal).length() < 1e-5, "normal distinta en vértice duplicado");
    }
}

#[test]
fn obj_basic_formats() {
    let text = "# comentario\n\nv 0 0 0\nv 1 0 0\nv 1 1 0 # esquina\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\nvn 0 0 1\n\
f 1/1/1 2/2/1 3/3/1 4/4/1\nf -4 -3 -2\nf 1//1 3//1 4//1\n";
    let m = parse_obj(text).unwrap();
    // quad -> 2 tris, + 1 + 1
    assert_eq!(m.triangle_count(), 4);
    // combinaciones únicas (v,vt,vn): 4 + 3 (solo v) + 3 (v//vn) = 10
    assert_eq!(m.vertices.len(), 10);
    for v in &m.vertices {
        assert_eq!(v.color, [1.0, 1.0, 1.0]);
        assert!((v.normal.length() - 1.0).abs() < 1e-5);
    }
    assert!(m.vertices.iter().all(|v| v.normal.z > 0.99));
    assert!((m.bounds_center - Vec3::new(0.5, 0.5, 0.0)).length() < 1e-6);
    assert!((m.bounds_radius - 0.5f32.sqrt()).abs() < 1e-5);
}

#[test]
fn obj_polygon_fan_vertex_colors_and_smooth_normals() {
    let text = "v 0 0 0 1 0 0\nv 1 0 0 0 1 0\nv 1 1 0 0 0 1\nv 0 1 0\nf 1 2 3 4\r\n";
    let m = parse_obj(text).unwrap();
    assert_eq!(m.triangle_count(), 2);
    assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
    assert_eq!(m.vertices[0].color, [1.0, 0.0, 0.0]);
    assert_eq!(m.vertices[3].color, [1.0, 1.0, 1.0]);
    for v in &m.vertices {
        assert!((v.normal - Vec3::Z).length() < 1e-5);
    }
}

#[test]
fn obj_errors_without_panic() {
    let bad = [
        "",
        "v 0 0 0\n",
        "v 0 0\nf 1 1 1\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 4\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 0 1 2\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 x\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf -4 1 2\n",
        "v a b c\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1/1 2 3\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1//1 2 3\n",
        "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1/2/3/4 2 3\n",
        "v nan 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
    ];
    for b in bad {
        let e = parse_obj(b);
        assert!(e.is_err(), "debería fallar: {b:?}");
        assert!(!e.unwrap_err().to_string().is_empty());
    }
    assert!(load_obj("/ruta/que/no/existe.obj").is_err());
}

#[test]
fn obj_from_file() {
    let path = format!("{SCRATCH}/geometry_test_tri.obj");
    std::fs::create_dir_all(SCRATCH).unwrap();
    std::fs::write(&path, "o tri\nv 0 0 0\nv 2 0 0\nv 0 2 0\nvt 0 0\nvt 1 0\nvt 0 1\nvn 0 0 1\ns off\nf 1/1/1 2/2/1 3/3/1\n").unwrap();
    let m = load_obj(&path).unwrap();
    assert_eq!(m.vertices.len(), 3);
    assert_eq!(m.triangle_count(), 1);
    assert_eq!(m.vertices[1].uv, [1.0, 0.0]);
    assert!(m.bounds_radius > 1.0);
}

#[test]
fn bounds_empty_and_simple() {
    let mut m = Mesh::default();
    m.update_bounds();
    assert_eq!(m.bounds_radius, 0.0);
    let c = cube_mesh();
    assert!((c.bounds_radius - 0.75f32.sqrt()).abs() < 1e-5);
    assert_eq!(c.bounds_center, Vec3::ZERO);
}

fn assert_faces_match_normals(m: &Mesh) {
    for i in 0..m.triangle_count() {
        let [a, b, c] = tri(m, i);
        let g = (b - a).cross(c - a).normalized();
        let t = &m.indices[i * 3..i * 3 + 3];
        for &k in t {
            assert!(g.dot(m.vertices[k as usize].normal) > 0.99, "tri {i}");
        }
    }
}

#[test]
fn cube_and_quad_winding() {
    let c = cube_mesh();
    assert_eq!(c.triangle_count(), 12);
    assert_eq!(c.vertices.len(), 24);
    assert_faces_match_normals(&c);
    for i in 0..c.triangle_count() {
        let [a, b, d] = tri(&c, i);
        assert!((b - a).cross(d - a).dot(a + b + d) > 0.0);
    }
    let q = quad_mesh();
    assert_eq!(q.triangle_count(), 2);
    assert_faces_match_normals(&q);
}

#[test]
fn ring_double_sided() {
    let r = ring_mesh(1.5, 2.5, 32);
    assert_eq!(r.triangle_count(), 2 * 2 * 32);
    assert_faces_match_normals(&r);
    let (mut up, mut down) = (0, 0);
    for i in 0..r.triangle_count() {
        let [a, b, c] = tri(&r, i);
        let n = (b - a).cross(c - a);
        assert!(n.length() > 1e-6);
        assert!(a.y == 0.0 && b.y == 0.0 && c.y == 0.0);
        if n.y > 0.0 { up += 1 } else { down += 1 }
    }
    assert_eq!(up, down);
    for v in &r.vertices {
        let rad = (v.position.x.powi(2) + v.position.z.powi(2)).sqrt();
        assert!(rad >= 1.5 - 1e-5 && rad <= 2.5 + 1e-5);
        assert!((v.uv[0] - (rad - 1.5)).abs() < 1e-4);
        assert!(v.uv[1] >= 0.0 && v.uv[1] <= 1.0);
    }
    assert!((r.bounds_radius - 2.5).abs() < 1e-4);
    // Parámetros degenerados no entran en pánico.
    assert!(ring_mesh(2.0, 1.0, 0).triangle_count() > 0);
}

fn setup() -> SphereLods {
    SphereLods::new(5, &|d| 0.02 * (4.0 * d.x).sin())
}

#[test]
fn lod_levels() {
    let l = setup();
    assert_eq!(l.levels.len(), 6);
    assert_eq!(l.levels[0].triangle_count(), 20);
    assert_eq!(l.levels[5].triangle_count(), 20480);
}

#[test]
fn lod_select_monotonic_and_bounded_error() {
    let l = setup();
    let (r, fov, h) = (1.0f32, 1.0f32, 720.0f32);
    let mut prev = 0usize;
    let mut d = 1000.0f32;
    while d > 1.0 {
        let lv = l.select(d, r, fov, h);
        assert!(lv >= prev, "d={d}");
        if lv < 5 {
            assert!(l.error_px(lv, d, r, fov, h) <= 2.0 + 1e-3, "d={d} lv={lv}");
        }
        prev = lv;
        d *= 0.97;
    }
    assert_eq!(l.select(1000.0, r, fov, h), 0);
    assert_eq!(l.select(0.5, r, fov, h), 5); // dentro de la esfera
    assert_eq!(l.select(f32::NAN, r, fov, h), 5);
    assert_eq!(l.select(10.0, r, fov, 0.0), 5);
}

#[test]
fn lod_hysteresis() {
    let l = setup();
    let (r, fov, h) = (1.0f32, 1.0f32, 720.0f32);
    // Con margen 0 coincide con select.
    for &d in &[2.0f32, 5.0, 20.0, 80.0, 300.0] {
        let s = l.select(d, r, fov, h);
        for cur in 0..6 {
            assert_eq!(l.select_with_hysteresis(cur, d, r, fov, h, 0.0), s);
        }
    }
    // Con margen, existe una banda donde se conserva el nivel actual y nunca queda peor que 2*(1+m) px.
    let mut kept = 0;
    let mut d = 2.0f32;
    while d < 400.0 {
        let s = l.select(d, r, fov, h);
        for cur in 0..6usize {
            let hl = l.select_with_hysteresis(cur, d, r, fov, h, 0.3);
            if hl != s {
                kept += 1;
            }
            if hl < 5 {
                assert!(l.error_px(hl, d, r, fov, h) <= 2.0 * 1.3 + 1e-3);
            }
        }
        d *= 1.05;
    }
    assert!(kept > 0);
}
