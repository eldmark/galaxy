use super::*;
use crate::math::Mat4;
use crate::terrain::cubesphere::dir_to_face_uv;
use crate::world::{solar_system, terrain_altitude};

fn models() -> Arc<Models> {
    Arc::new(Models::load("assets/models").expect("assets/models"))
}

fn scatter(kind: BodyKind, seed: u64) -> Scatter {
    let b = solar_system().into_iter().find(|b| b.kind == kind).unwrap();
    Scatter::new(kind, b.radius, seed, models()).unwrap()
}

/// Chunk de nivel `level` que contiene la dirección `d`.
fn chunk_at(d: DVec3, level: u8) -> ChunkKey {
    let (face, u, v) = dir_to_face_uv(d);
    let n = (1u32 << level) as f64;
    let c = |t: f64| ((((t * 0.5 + 0.5) * n) as u32).min(n as u32 - 1));
    ChunkKey { face, level, x: c(u), y: c(v) }
}

const KINDS: [BodyKind; 4] = [BodyKind::Verdalia, BodyKind::Fornax, BodyKind::Nix, BodyKind::Mutante];

/// Todas las instancias de muchas celdas repartidas por el planeta.
fn sample_instances(s: &Scatter, n: usize) -> Vec<Instance> {
    let mut out = Vec::new();
    let golden = std::f64::consts::PI * (3.0 - 5f64.sqrt());
    for i in 0..n {
        let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
        let r = (1.0 - y * y).sqrt();
        let d = DVec3::new(r * (golden * i as f64).cos(), y, r * (golden * i as f64).sin());
        out.extend(s.instances(chunk_at(d, CELL_LEVEL)));
    }
    out
}

#[test]
fn only_four_bodies_have_species() {
    let b = solar_system();
    for body in &b {
        let has = Scatter::new(body.kind, body.radius, 1, models()).is_some();
        assert_eq!(has, KINDS.contains(&body.kind), "{}", body.name);
    }
}

#[test]
fn same_seed_same_positions_any_order() {
    for kind in KINDS {
        let a = scatter(kind, 42);
        let spot = a.find_dense_spot(300);
        let keys: Vec<ChunkKey> = (6..=8).map(|l| chunk_at(spot, l)).collect();
        let fwd: Vec<Vec<Instance>> = keys.iter().map(|k| a.instances(*k)).collect();
        // otro Scatter, claves en orden inverso y con otras claves intercaladas
        let b = scatter(kind, 42);
        let _ = b.instances(chunk_at(spot * -1.0, 8));
        let mut rev: Vec<Vec<Instance>> = keys.iter().rev().map(|k| b.instances(*k)).collect();
        rev.reverse();
        assert!(!fwd[2].is_empty(), "{kind:?}: la celda densa debe tener instancias");
        for (x, y) in fwd.iter().zip(&rev) {
            assert_eq!(x.len(), y.len());
            for (p, q) in x.iter().zip(y) {
                assert_eq!(p.pos, q.pos);
                assert_eq!(p.yaw, q.yaw);
                assert_eq!(p.scale, q.scale);
            }
        }
    }
}

#[test]
fn different_seed_different_positions() {
    for kind in KINDS {
        let a = scatter(kind, 1);
        let spot = a.find_dense_spot(300);
        let key = chunk_at(spot, 7);
        let (pa, pb): (Vec<_>, Vec<_>) = {
            let b = scatter(kind, 2);
            (a.instances(key).iter().map(|i| (i.pos.x.to_bits(), i.pos.y.to_bits(), i.pos.z.to_bits())).collect(), b.instances(key).iter().map(|i| (i.pos.x.to_bits(), i.pos.y.to_bits(), i.pos.z.to_bits())).collect())
        };
        assert_ne!(pa, pb, "{kind:?}: otra semilla debe dar otras posiciones");
    }
}

#[test]
fn level_aggregation_matches_cells() {
    // un chunk de nivel 6 contiene exactamente las instancias de sus 16 celdas de nivel 8
    let s = scatter(BodyKind::Verdalia, 3);
    let spot = s.find_dense_spot(300);
    let big = s.instances(chunk_at(spot, 6));
    let mut sum = 0;
    for c in Scatter::cells_of(chunk_at(spot, 6)) {
        sum += s.placer.cell_instances(c).len();
    }
    assert_eq!(big.len(), sum);
    assert!(big.len() >= s.instances(chunk_at(spot, 8)).len());
}

#[test]
fn verdalia_has_no_instances_underwater() {
    let s = scatter(BodyKind::Verdalia, 7);
    let all = sample_instances(&s, 4000);
    assert!(all.len() > 200, "debe haber árboles ({})", all.len());
    for i in &all {
        let alt = s.placer.surface_radius(i.dir) - s.placer.radius;
        assert!(alt > 0.0, "árbol bajo el nivel del mar: alt {alt}");
        assert!(crate::relief::height(BodyKind::Verdalia, i.dir.to_f32()) > 0.0);
    }
}

#[test]
fn slope_never_exceeds_species_limit() {
    for kind in KINDS {
        let s = scatter(kind, 5);
        let all = sample_instances(&s, 3000);
        assert!(!all.is_empty(), "{kind:?}: sin instancias");
        for i in &all {
            let sp = &s.placer.species[i.species as usize];
            let ang = (i.up.to_f64().dot(i.dir)).clamp(-1.0, 1.0).acos().to_degrees() as f32;
            assert!(ang <= sp.max_slope_deg + 0.05, "{kind:?}: pendiente {ang} > {}", sp.max_slope_deg);
            assert!(i.scale >= sp.scale.0 - 1e-5 && i.scale <= sp.scale.1 + 1e-5);
        }
    }
}

#[test]
fn local_up_axis_is_the_surface_normal_and_basis_is_orthonormal() {
    let s = scatter(BodyKind::Fornax, 9);
    let all = sample_instances(&s, 1500);
    assert!(!all.is_empty());
    for i in all.iter().take(300) {
        let (x, y, z) = i.basis();
        assert!((y - i.up).length() < 1e-6, "el eje Y local es la normal");
        assert!(x.dot(y).abs() < 1e-4 && x.dot(z).abs() < 1e-4 && y.dot(z).abs() < 1e-4);
        assert!((x.length() - 1.0).abs() < 1e-4 && (z.length() - 1.0).abs() < 1e-4);
        assert!(x.cross(y).dot(z) > 0.999, "mano derecha");
        // la normal es la del relieve: diferencias de relief::height en dos direcciones tangentes
        let n = {
            let t = |d: DVec3| d * s.placer.surface_radius(d);
            let up = if i.dir.y.abs() < 0.9 { DVec3::new(0.0, 1.0, 0.0) } else { DVec3::new(1.0, 0.0, 0.0) };
            let a = crate::terrain::cubesphere::cross(up, i.dir).normalized();
            let b = crate::terrain::cubesphere::cross(i.dir, a).normalized();
            let e = 1.5 / s.placer.radius;
            let (pa, pb) = (t((i.dir + a * e).normalized()) - t((i.dir - a * e).normalized()), t((i.dir + b * e).normalized()) - t((i.dir - b * e).normalized()));
            let mut n = crate::terrain::cubesphere::cross(pa, pb).normalized();
            if n.dot(i.dir) < 0.0 {
                n = n * -1.0;
            }
            n
        };
        assert!(n.dot(i.up.to_f64()) > 0.995, "normal independiente coincide ({})", n.dot(i.up.to_f64()));
    }
}

#[test]
fn baked_instances_stand_on_the_ground_along_the_normal() {
    let s = scatter(BodyKind::Verdalia, 7);
    let spot = s.find_dense_spot(300);
    let inst = s.instances(chunk_at(spot, 8));
    assert!(!inst.is_empty());
    let one = [inst[0]];
    let (mesh, _) = s.bake(&one, Lod::High);
    let i = one[0];
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for v in &mesh.vertices {
        let h = (v.position.to_f64() - i.pos).dot(i.up.to_f64());
        lo = lo.min(h);
        hi = hi.max(h);
    }
    assert!(lo.abs() < 0.02, "la base del modelo queda en el plano del suelo (lo={lo})");
    let model_h = s.models.high[0].vertices.iter().map(|v| v.position.y).fold(0.0f32, f32::max) as f64;
    assert!((hi - model_h * i.scale as f64).abs() < 0.02, "altura = escala * altura del modelo");
    // la base está pegada al relieve (hundida `sink_m` como mucho)
    let sp = &s.placer.species[0];
    let gap = (i.pos.length() - s.placer.surface_radius(i.dir)).abs();
    assert!(gap <= sp.sink_m as f64 + 1e-3, "gap {gap}");
}

#[test]
fn counts_are_bounded_per_chunk() {
    for kind in KINDS {
        let s = scatter(kind, 11);
        let spot = s.find_dense_spot(300);
        for l in N_SCATTER..=CELL_LEVEL {
            let n = s.instances(chunk_at(spot, l)).len();
            let max = MAX_PER_CELL * 4usize.pow((CELL_LEVEL - l) as u32);
            assert!(n <= max, "{kind:?} nivel {l}: {n} > {max}");
        }
        for i in 0..40 {
            let d = DVec3::new((i as f64 * 0.7).cos(), (i as f64 * 0.3).sin(), (i as f64 * 1.3).cos()).normalized();
            assert!(s.placer.cell_instances(chunk_at(d, CELL_LEVEL)).len() <= MAX_PER_CELL);
        }
    }
}

#[test]
fn building_footprint_is_flat_and_axis_aligned() {
    let s = scatter(BodyKind::Mutante, 4);
    let all = sample_instances(&s, 4000);
    assert!(!all.is_empty());
    let (mut bad, mut total) = (0, 0);
    for i in &all {
        // yaw múltiplo de 90 grados
        let q = i.yaw / std::f32::consts::FRAC_PI_2;
        assert!((q - q.round()).abs() < 1e-4);
        // suelo plano a +-3.6 m: ningún escalón (el relieve de Mutante es de peldaños de ~6 m)
        let sp = &s.placer.species[0];
        let r0 = s.placer.surface_radius(i.dir);
        let (a, b) = (i.dir.normalized(), crate::terrain::cubesphere::cross(i.dir, DVec3::new(0.0, 1.0, 0.0)).normalized());
        for k in [1.0, -1.0] {
            let d = (a + b * (k * sp.foot_m as f64 / s.placer.radius)).normalized();
            total += 1;
            if (s.placer.surface_radius(d) - r0).abs() > 0.8 {
                bad += 1;
            }
        }
    }
    // el relieve son peldaños de ~6 m: un edificio sobre un escalón fallaría por metros; se tolera algún borde de huella
    assert!(bad * 25 <= total, "edificios sobre escalones: {bad}/{total}");
}

#[test]
fn lod_and_distance_limit_change_triangle_count() {
    let mut s = scatter(BodyKind::Verdalia, 7);
    s.set_bake_budget(10_000);
    let spot = s.find_dense_spot(400);
    // chunks de nivel 8 alrededor del punto (hojas del terreno simuladas)
    let n = 1u32 << CELL_LEVEL;
    let c = chunk_at(spot, CELL_LEVEL);
    let mut keys = Vec::new();
    for dy in -12i32..=12 {
        for dx in -12i32..=12 {
            let (x, y) = (c.x as i32 + dx, c.y as i32 + dy);
            if x >= 0 && y >= 0 && x < n as i32 && y < n as i32 {
                keys.push(ChunkKey { face: c.face, level: CELL_LEVEL, x: x as u32, y: y as u32 });
            }
        }
    }
    let cam = spot * (s.placer.surface_radius(spot) + 5.0);
    s.use_lod = true;
    let out_lod = s.frame(&keys, cam);
    let with_lod = s.stats.clone();
    assert_eq!(out_lod.len() as u32, with_lod.draw_calls);
    s.use_lod = false;
    let _ = s.frame(&keys, cam);
    let no_lod = s.stats.clone();
    assert!(with_lod.instances_high > 0 && with_lod.instances_low > 0, "{with_lod:?}");
    assert!(with_lod.triangles < no_lod.triangles, "LOD reduce triángulos: {} vs {}", with_lod.triangles, no_lod.triangles);
    assert_eq!(with_lod.triangles_without_lod, no_lod.triangles);
    assert!(with_lod.instances_culled_distance > 0, "la distancia máxima descarta instancias: {with_lod:?}");
    assert_eq!(with_lod.instances_shown() + with_lod.instances_culled_distance, with_lod.instances_total);
    // cámara lejos: no se dibuja nada y todo cuenta como descartado o fuera de rango
    s.use_lod = true;
    let far = spot * (s.placer.surface_radius(spot) + 3000.0);
    let out = s.frame(&keys, far);
    assert!(out.is_empty());
    assert_eq!(s.stats.triangles, 0);
    assert_eq!(s.stats.chunks_out_of_range as usize, keys.len());
    // segunda llamada con la misma cámara: todo sale de la caché
    let _ = s.frame(&keys, cam);
    let _ = s.frame(&keys, cam);
    assert_eq!(s.stats.baked, 0);
}

#[test]
fn low_poly_meshes_are_cheaper_and_face_outward() {
    let m = models();
    for id in ModelId::ALL {
        let (hi, lo) = (&m.high[id.index()], &m.low[id.index()]);
        assert!(lo.triangle_count() * 3 <= hi.triangle_count(), "{id:?}: {} vs {}", lo.triangle_count(), hi.triangle_count());
        let c = lo.bounds_center;
        let mut outward = 0;
        for t in lo.indices.chunks_exact(3) {
            let (a, b, cc) = (lo.vertices[t[0] as usize].position, lo.vertices[t[1] as usize].position, lo.vertices[t[2] as usize].position);
            let n = (b - a).cross(cc - a);
            if n.dot((a + b + cc) * (1.0 / 3.0) - c) >= -1e-4 {
                outward += 1;
            }
        }
        assert!(outward * 100 >= lo.triangle_count() * 80, "{id:?}: {outward}/{}", lo.triangle_count());
        let ymin = lo.vertices.iter().map(|v| v.position.y).fold(f32::MAX, f32::min);
        assert!(ymin.abs() < 1e-5, "origen en la base");
    }
}

/// Instancias asentadas a lo largo del tiempo: al rotar el planeta, la distancia local al terreno no cambia.
#[test]
fn instances_stay_on_the_ground_while_the_planet_rotates() {
    let bodies = solar_system();
    let body = bodies.iter().find(|b| b.kind == BodyKind::Verdalia).unwrap();
    let s = scatter(BodyKind::Verdalia, 7);
    let spot = s.find_dense_spot(300);
    let inst = s.instances(chunk_at(spot, 8));
    assert!(!inst.is_empty());
    let (mesh, _) = s.bake(&inst[..1], Lod::High);
    let base_v = mesh.vertices[0].position;
    // distancia local vértice-terreno (marco fijo al planeta): independiente del tiempo
    let local_gap = base_v.to_f64().length() - s.placer.surface_radius(base_v.to_f64().normalized());
    let mut positions = Vec::new();
    for t in [0.0, 13.7, 211.0, 1234.5, 5000.0] {
        // dibujado: model = traslación(planeta - ojo) * rotación_y(ángulo)
        let eye = body.position(t) + body.fixed_to_world(t, spot * (s.placer.surface_radius(spot) + 40.0));
        let model = Mat4::translation((body.position(t) - eye).to_f32()) * Mat4::rotation_y(body.rotation_angle(t) as f32);
        let rel = model.transform_point(base_v);
        let world = eye + rel.to_f64();
        // distancia de ese punto del mundo al terreno, medida con la física (relieve + rotación en el instante t)
        let gap = terrain_altitude(body, t, world);
        assert!((gap - local_gap).abs() < 0.05, "t={t}: gap {gap} vs local {local_gap}");
        // el eje Y local sigue siendo la normal: en el mundo apunta radialmente hacia afuera (a la pendiente)
        let up_w = body.fixed_to_world(t, inst[0].up.to_f64());
        let radial = (world - body.position(t)).normalized();
        assert!(up_w.dot(radial) > 0.9);
        positions.push(world - body.position(t));
    }
    // las posiciones en el mundo SÍ cambian (el planeta gira) pero la distancia al centro no
    assert!((positions[0] - positions[2]).length() > 100.0);
    assert!((positions[0].length() - positions[3].length()).abs() < 0.05);
}

#[test]
fn global_seed_is_configurable() {
    set_global_seed(99);
    assert_eq!(global_seed(), 99);
    set_global_seed(1);
}

#[test]
#[ignore = "medida de densidad: cargo test densities -- --ignored --nocapture"]
fn densities() {
    for kind in KINDS {
        let s = scatter(kind, 1);
        let all = sample_instances(&s, 2000);
        let cells = 2000.0;
        println!("{kind:?}: {:.2} instancias por celda de {:.1} m ({} en total)", all.len() as f64 / cells, s.placer.cell_len_m(), all.len());
    }
}
