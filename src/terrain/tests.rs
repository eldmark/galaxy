use super::chunk::grid_area;
use super::*;
use crate::math::DVec3;
use crate::world::BodyKind;

const R: f64 = 3500.0;
const FOV: f64 = 1.0;
const VH: f64 = 540.0;

fn flat_terrain(max_level: u8) -> Terrain {
    Terrain::with_height(BodyKind::Verdalia, R, max_level, 0.0, Arc::new(|_| 0.0))
}

/// Altura de prueba propia (independiente de `relief`): colinas suaves de +-1 %.
fn hills(d: Vec3) -> f32 {
    0.01 * ((d.x * 13.0).sin() * (d.y * 9.0).cos() + (d.z * 17.0 + d.x * 5.0).sin()) * 0.5
}
fn hilly_terrain(max_level: u8) -> Terrain {
    Terrain::with_height(BodyKind::Verdalia, R, max_level, 0.011, Arc::new(hills))
}

fn cam_at(alt: f64) -> DVec3 {
    DVec3::new(0.3, 0.8, 0.52).normalized() * (R + alt)
}

fn has_overlap(keys: &[ChunkKey]) -> bool {
    let set: HashSet<_> = keys.iter().copied().collect();
    keys.iter().any(|k| {
        let mut a = *k;
        while let Some(p) = a.parent() {
            a = p;
            if set.contains(&a) {
                return true;
            }
        }
        false
    })
}

fn leaf_tris(n: usize) -> usize {
    n * 640
}

#[test]
fn leaves_cover_sphere_without_overlap() {
    let t = flat_terrain(8);
    let exact = 4.0 * std::f64::consts::PI * R * R;
    // cámara dentro de la esfera: no hay horizonte, se seleccionan hojas de toda la esfera
    for cam in [DVec3::ZERO, cam_at(-300.0), DVec3::new(100.0, -200.0, 3300.0)] {
        let (keys, _) = t.tree.select(cam, FOV, VH, None);
        assert!(!has_overlap(&keys));
        let area: f64 = keys.iter().map(|k| grid_area(&t.build(*k))).sum();
        assert!((area - exact).abs() / exact < 0.003, "área {area} vs {exact} ({} hojas)", keys.len());
    }
}

#[test]
fn level_follows_distance() {
    let t = flat_terrain(8);
    let near = t.select(cam_at(20.0), FOV, VH, None);
    assert_eq!(near.iter().map(|k| k.level).max(), Some(8));
    let far = t.select(cam_at(3.0 * R), FOV, VH, None);
    assert!(far.iter().all(|k| k.level <= 3), "lejos no debe haber niveles altos");
    assert!(far.iter().map(|k| k.level).min().unwrap() >= t.tree.min_level);
    // el nivel baja al alejarse y sube al acercarse
    let lv = |alt: f64| t.select(cam_at(alt), FOV, VH, None).iter().map(|k| k.level).max().unwrap();
    assert!(lv(20.0) >= lv(300.0) && lv(300.0) >= lv(3000.0) && lv(3000.0) >= lv(3.0 * R));
}

#[test]
fn triangle_count_bounded_while_descending() {
    let t = hilly_terrain(8);
    let mut worst = 0;
    let mut worst_cull = 0;
    let mut alt = 3.0 * R;
    while alt > 20.0 {
        let cam = cam_at(alt);
        let fwd = (DVec3::new(0.0, 0.0, 0.0) - cam).normalized(); // mirando al centro
        let up = DVec3::new(0.0, 1.0, 0.0);
        let fr = Frustum::from_camera(cam, fwd, up, FOV, 16.0 / 9.0);
        let n = t.select(cam, FOV, VH, None).len();
        let nf = t.select(cam, FOV, VH, Some(&fr)).len();
        println!("alt {alt:.0}: sin frustum {} hojas, con frustum {}", n, nf);
        worst = worst.max(leaf_tris(n));
        worst_cull = worst_cull.max(leaf_tris(nf));
        alt *= 0.9;
    }
    println!("peor caso sin frustum: {worst} triángulos; con frustum: {worst_cull}");
    assert!(worst_cull < 150_000, "{worst_cull}");
}

#[test]
fn horizon_discards_far_side() {
    let t = flat_terrain(8);
    for alt in [20.0, 300.0, 3000.0, 10000.0] {
        let cam = DVec3::new(0.0, 0.0, R + alt);
        let (keys, st) = t.tree.select(cam, FOV, VH, None);
        let hor = (R / (R + alt)).acos();
        for k in &keys {
            let g = t.tree.geom(k);
            let a = g.cdir.z.clamp(-1.0, 1.0).acos();
            assert!(a - g.ang <= hor + 1e-9, "alt {alt}: hoja tras el horizonte");
            assert!(g.cdir.z > -0.2 || alt > 5000.0, "alt {alt}: hoja del lado opuesto");
        }
        assert!(st.culled_horizon > 0);
    }
}

#[test]
fn frustum_culls_behind_camera() {
    let t = flat_terrain(8);
    let cam = cam_at(2000.0);
    let up = DVec3::new(0.0, 1.0, 0.0);
    let away = cam.normalized(); // mirando hacia el espacio
    let fr = Frustum::from_camera(cam, away, up, FOV, 1.6);
    let (keys, st) = t.tree.select(cam, FOV, VH, Some(&fr));
    assert!(keys.is_empty() && st.culled_frustum > 0, "{} hojas", keys.len());
    let down = Frustum::from_camera(cam, cam.normalized() * -1.0, up, FOV, 1.6);
    assert!(!t.tree.select(cam, FOV, VH, Some(&down)).0.is_empty());
}

#[test]
fn frame_has_no_holes_with_tiny_budget_and_converges() {
    let mut t = hilly_terrain(8);
    t.budget = 3;
    let cam = cam_at(120.0);
    let fr = Frustum::from_camera(cam, cam.normalized() * -1.0, DVec3::new(0.0, 1.0, 0.0), FOV, 16.0 / 9.0);
    let fr = Some(&fr);
    let (leaves, _) = t.tree.select(cam, FOV, VH, fr);
    let mut prev_fallback = u32::MAX;
    for f in 0..400 {
        let out = t.frame(cam, FOV, VH, fr);
        let keys: Vec<_> = out.iter().map(|o| o.0).collect();
        assert!(!has_overlap(&keys), "solape en cuadro {f}");
        // cobertura: cada hoja seleccionada queda dentro de algún chunk mostrado (o es uno)
        for l in &leaves {
            let covered = keys.iter().any(|k| k == l || k.is_ancestor_of(l));
            assert!(covered, "hueco en cuadro {f}");
        }
        assert!(t.stats.generated as usize <= t.budget + 24 * 4 + 6 * 16 + 1000);
        prev_fallback = t.stats.fallback;
        if prev_fallback == 0 {
            break;
        }
    }
    assert_eq!(prev_fallback, 0, "no converge");
}

#[test]
fn cache_respects_capacity_and_never_evicts_current() {
    let mut t = hilly_terrain(8);
    t.set_cache_capacity(200);
    t.budget = 64;
    for i in 0..60 {
        // recorre la superficie rápido para llenar la caché
        let ang = i as f64 * 0.2;
        let cam = DVec3::new(ang.cos(), 0.3, ang.sin()).normalized() * (R + 80.0);
        let out = t.frame(cam, FOV, VH, None);
        assert!(!out.is_empty());
        assert!(t.cache_len() <= 200.max(t.stats.drawn as usize));
    }
}

#[test]
fn chunks_are_deterministic_and_finite() {
    let t = hilly_terrain(8);
    let k = ChunkKey { face: 3, level: 6, x: 13, y: 40 };
    let (a, b) = (t.build(k), t.build(k));
    assert_eq!(a.indices, b.indices);
    for (p, q) in a.vertices.iter().zip(&b.vertices) {
        assert_eq!(p.position, q.position);
        assert_eq!(p.normal, q.normal);
        assert!(p.position.x.is_finite() && p.normal.x.is_finite());
    }
}

/// Las normales coinciden entre niveles: el vértice de un hijo en la misma posición que uno del padre
/// tiene (casi) la misma normal, y las normales a través de la arista entre dos caras del cubo también.
#[test]
fn normals_continuous_across_levels_and_faces() {
    let t = hilly_terrain(8);
    let p = ChunkKey { face: 2, level: 4, x: 3, y: 9 };
    let pm = t.build(p);
    for c in p.children() {
        let cm = t.build(c);
        let (ox, oy) = ((c.x & 1) as usize * 8, (c.y & 1) as usize * 8);
        for j in 0..=8 {
            for i in 0..=8 {
                let vp = pm.vertices[(oy + j) * 17 + ox + i];
                let vc = cm.vertices[(2 * j) * 17 + 2 * i];
                assert!((vp.position - vc.position).length() < 1e-2);
                assert!(vp.normal.dot(vc.normal) > 0.999, "{}", vp.normal.dot(vc.normal));
            }
        }
    }
    // arista +u de la cara 0 con su vecina: puntos de la arista, normales calculadas desde ambas caras
    let a = t.build(ChunkKey { face: 4, level: 3, x: 7, y: 2 }); // borde u = +1 de la cara +Z
    for j in 0..17 {
        let va = a.vertices[j * 17 + 16];
        let dir = va.position.to_f64().normalized();
        let (u2, v2) = cubesphere::dir_to_uv_on_face(0, dir);
        let n2 = chunk::surface_normal(0, u2, v2, 1e-4, R, &hills);
        assert!(va.normal.to_f64().dot(n2) > 0.999);
    }
}

#[test]
fn no_nan_anywhere() {
    let mut t = hilly_terrain(8);
    t.budget = 500;
    for alt in [5.0, 30.0, 500.0, 5000.0, 20000.0] {
        for out in t.frame(cam_at(alt), FOV, VH, None) {
            for v in &out.1.vertices {
                assert!(v.position.length().is_finite() && v.normal.length().is_finite());
            }
        }
    }
}
