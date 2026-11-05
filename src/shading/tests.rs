use super::*;
use crate::math::{Mat4, Vec3};

const ALL: [BodyKind; 6] = [
    BodyKind::Sun,
    BodyKind::Verdalia,
    BodyKind::Siroco,
    BodyKind::Mutante,
    BodyKind::Nix,
    BodyKind::Fornax,
];

fn uniforms(light: Vec3, time: f32, detail: f32) -> Uniforms {
    Uniforms { light_dir: light.normalized(), time, detail, model: Mat4::IDENTITY }
}

fn frag(p: Vec3, view: Vec3) -> FragIn {
    FragIn { p_local: p, normal: p, uv: [0.5, 0.5], color: [0.8, 0.7, 0.6], view_dir: view, distance: 10.0 }
}

/// Malla de puntos sobre la esfera (espiral de Fibonacci).
fn sphere_points(n: usize) -> Vec<Vec3> {
    (0..n)
        .map(|i| {
            let y = 1.0 - 2.0 * (i as f32 + 0.5) / n as f32;
            let r = (1.0 - y * y).sqrt();
            let a = i as f32 * 2.399_963;
            Vec3::new(r * a.cos(), y, r * a.sin())
        })
        .collect()
}

fn render_all(sh: &dyn FragmentShader, u: &Uniforms, pts: &[Vec3]) -> Vec<[f32; 3]> {
    pts.iter()
        .map(|&p| {
            let view = (Vec3::new(0.0, 0.0, 5.0) - p).normalized();
            sh.shade(&frag(p, view), u)
        })
        .collect()
}

fn mean(v: &[[f32; 3]]) -> f32 {
    v.iter().map(|c| (c[0] + c[1] + c[2]) / 3.0).sum::<f32>() / v.len() as f32
}
fn diff(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter().zip(b).map(|(x, y)| (0..3).map(|i| (x[i] - y[i]).abs()).sum::<f32>()).sum::<f32>() / a.len() as f32
}

#[test]
fn todos_finitos_y_en_rango() {
    let pts = sphere_points(1500);
    let lights = [Vec3::new(1.0, 0.3, 0.5), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.1)];
    for kind in ALL {
        let sh = shader_for(kind);
        for t in [0.0, 0.37, 5.0, 123.4, 99999.0] {
            for l in lights {
                for detail in [1.0, 0.5, 0.1] {
                    for c in render_all(sh.as_ref(), &uniforms(l, t, detail), &pts) {
                        for v in c {
                            assert!(v.is_finite() && (0.0..=1.0).contains(&v), "{kind:?} t={t} -> {c:?}");
                        }
                    }
                }
            }
        }
    }
    for sh in [shader_for_ring(), ship_shader()] {
        for c in render_all(sh.as_ref(), &uniforms(Vec3::new(1.0, 1.0, 0.0), 1.0, 1.0), &pts) {
            assert!(c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
        }
    }
}

#[test]
fn animados_cambian_con_el_tiempo() {
    let pts = sphere_points(800);
    let l = Vec3::new(1.0, 0.5, 0.8);
    for kind in [BodyKind::Sun, BodyKind::Fornax, BodyKind::Siroco, BodyKind::Verdalia, BodyKind::Mutante] {
        let sh = shader_for(kind);
        let a = render_all(sh.as_ref(), &uniforms(l, 1.0, 1.0), &pts);
        let b = render_all(sh.as_ref(), &uniforms(l, 7.0, 1.0), &pts);
        assert!(diff(&a, &b) > 1e-3, "{kind:?} no cambia con el tiempo");
    }
}

#[test]
fn estaticos_no_cambian_con_el_tiempo() {
    let pts = sphere_points(400);
    let l = Vec3::new(1.0, 0.5, 0.8);
    for sh in [shader_for_ring(), ship_shader()] {
        let a = render_all(sh.as_ref(), &uniforms(l, 1.0, 1.0), &pts);
        let b = render_all(sh.as_ref(), &uniforms(l, 50.0, 1.0), &pts);
        assert_eq!(a, b);
    }
    let a = sky_color(Vec3::new(0.3, 0.2, 0.9), 0.0);
    assert_eq!(a, sky_color(Vec3::new(0.3, 0.2, 0.9), 77.0));
}

#[test]
fn mismo_tiempo_mismo_resultado() {
    let pts = sphere_points(300);
    let l = Vec3::new(0.2, 0.5, 0.8);
    for kind in ALL {
        let s1 = shader_for(kind);
        let s2 = shader_for(kind);
        let a = render_all(s1.as_ref(), &uniforms(l, 13.3, 1.0), &pts);
        let b = render_all(s2.as_ref(), &uniforms(l, 13.3, 1.0), &pts);
        assert_eq!(a, b, "{kind:?}");
    }
}

#[test]
fn shaders_distintos_dan_salidas_distintas() {
    let pts = sphere_points(600);
    let u = uniforms(Vec3::new(1.0, 0.5, 0.8), 3.0, 1.0);
    let outs: Vec<_> = ALL.iter().map(|&k| render_all(shader_for(k).as_ref(), &u, &pts)).collect();
    for i in 0..outs.len() {
        for j in i + 1..outs.len() {
            assert!(diff(&outs[i], &outs[j]) > 0.05, "{:?} y {:?} se parecen", ALL[i], ALL[j]);
        }
    }
}

#[test]
fn el_sol_no_depende_de_la_luz() {
    let pts = sphere_points(500);
    let sh = shader_for(BodyKind::Sun);
    let a = render_all(sh.as_ref(), &uniforms(Vec3::new(1.0, 0.0, 0.0), 2.0, 1.0), &pts);
    let b = render_all(sh.as_ref(), &uniforms(Vec3::new(-0.3, -1.0, 0.4), 2.0, 1.0), &pts);
    assert_eq!(a, b);
}

#[test]
fn los_planetas_dependen_de_la_luz() {
    let pts = sphere_points(500);
    for kind in &ALL[1..] {
        let sh = shader_for(*kind);
        let a = render_all(sh.as_ref(), &uniforms(Vec3::new(1.0, 0.0, 0.0), 2.0, 1.0), &pts);
        let b = render_all(sh.as_ref(), &uniforms(Vec3::new(-1.0, 0.0, 0.0), 2.0, 1.0), &pts);
        assert!(diff(&a, &b) > 0.02, "{kind:?}");
    }
}

#[test]
fn detail_bajo_cambia_poco_la_media() {
    let pts = sphere_points(3000);
    let l = Vec3::new(1.0, 0.4, 0.8);
    for kind in [BodyKind::Sun, BodyKind::Verdalia, BodyKind::Siroco, BodyKind::Nix, BodyKind::Fornax] {
        let sh = shader_for(kind);
        let hi = mean(&render_all(sh.as_ref(), &uniforms(l, 2.0, 1.0), &pts));
        let lo = mean(&render_all(sh.as_ref(), &uniforms(l, 2.0, 0.1), &pts));
        assert!((hi - lo).abs() < 0.08, "{kind:?}: {hi} vs {lo}");
    }
}

#[test]
#[ignore]
fn detail_bajo_cuesta_menos() {
    use std::time::Instant;
    let pts = sphere_points(40_000);
    let l = Vec3::new(1.0, 0.4, 0.8);
    for kind in ALL {
        let sh = shader_for(kind);
        let mut t = [0.0; 2];
        for (i, d) in [1.0, 0.1].iter().enumerate() {
            let t0 = Instant::now();
            let _ = render_all(sh.as_ref(), &uniforms(l, 2.0, *d), &pts);
            t[i] = t0.elapsed().as_secs_f32() * 1000.0;
        }
        println!("{kind:?}: detail 1.0 = {:.1} ms, detail 0.1 = {:.1} ms", t[0], t[1]);
    }
}

#[test]
fn cielo_estrellas_deterministas_y_variadas() {
    let mut lit = 0;
    let mut max = 0.0f32;
    let mut vals = std::collections::HashSet::new();
    for i in 0..200 {
        for j in 0..200 {
            let a = i as f32 / 200.0 * std::f32::consts::TAU;
            let b = (j as f32 / 200.0 - 0.5) * 2.5;
            let d = Vec3::new(a.cos() * b.cos(), b.sin(), a.sin() * b.cos());
            let c = sky_color(d, 0.0);
            assert_eq!(c, sky_color(d, 0.0));
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)));
            if c[0] + c[1] + c[2] > 0.3 {
                lit += 1;
                vals.insert((c[1] * 20.0) as i32);
            }
            max = max.max(c[1]);
        }
    }
    assert!(lit > 20 && lit < 4000, "estrellas visibles: {lit}");
    assert!(max > 0.6 && vals.len() > 4, "brillos poco variados");
    assert!(sky_color(Vec3::ZERO, 0.0).iter().all(|v| v.is_finite()));
}

// ---- Salida visual: un PNG por shader en el scratchpad ----

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (if crc & 1 == 1 { 0xedb8_8320 } else { 0 });
        }
    }
    !crc
}
fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend((data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend(data);
    out.extend(&body);
    out.extend(crc32(&body).to_be_bytes());
}
fn write_png(path: &str, w: usize, h: usize, rgb: &[u8]) {
    let mut raw = Vec::new();
    for y in 0..h {
        raw.push(0);
        raw.extend(&rgb[y * w * 3..(y + 1) * w * 3]);
    }
    let mut z = vec![0x78, 0x01];
    let mut chunks = raw.chunks(65535).peekable();
    while let Some(c) = chunks.next() {
        z.push(if chunks.peek().is_none() { 1 } else { 0 });
        z.extend((c.len() as u16).to_le_bytes());
        z.extend((!(c.len() as u16)).to_le_bytes());
        z.extend(c);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend((w as u32).to_be_bytes());
    ihdr.extend((h as u32).to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    std::fs::write(path, out).unwrap();
}

const OUT: &str = "/tmp/claude-1000/-home-dmark123-Documents-homework-graficas-galaxy/146d9493-3f05-4f8b-853c-4441b3071a9a/scratchpad";

fn to_byte(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Esfera analítica ortográfica de 256x256 vista desde +Z.
fn disc(sh: &dyn FragmentShader, u: &Uniforms, path: &str) {
    let n = 256;
    let mut img = vec![0u8; n * n * 3];
    for y in 0..n {
        for x in 0..n {
            let sx = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let sy = 1.0 - (y as f32 + 0.5) / n as f32 * 2.0;
            let (sx, sy) = (sx * 1.05, sy * 1.05);
            let z2 = 1.0 - sx * sx - sy * sy;
            let c = if z2 > 0.0 {
                sh.shade(&frag(Vec3::new(sx, sy, z2.sqrt()), Vec3::Z), u)
            } else {
                sky_color(Vec3::new(sx, sy, -1.0), 0.0)
            };
            img[(y * n + x) * 3..][..3].copy_from_slice(&to_byte(c));
        }
    }
    write_png(path, n, n, &img);
}

#[test]
fn genera_pngs_de_cada_shader() {
    if !std::path::Path::new(OUT).exists() {
        return;
    }
    let l = Vec3::new(-0.5, 0.4, 0.8);
    let names = ["ignea", "verdalia", "mutante", "siroco", "nix", "fornax"];
    let kinds = [BodyKind::Sun, BodyKind::Verdalia, BodyKind::Mutante, BodyKind::Siroco, BodyKind::Nix, BodyKind::Fornax];
    for (name, kind) in names.iter().zip(kinds) {
        disc(shader_for(kind).as_ref(), &uniforms(l, 3.0, 1.0), &format!("{OUT}/shader_{name}.png"));
    }
    disc(shader_for(BodyKind::Mutante).as_ref(), &uniforms(l, 12.0, 1.0), &format!("{OUT}/shader_mutante_cerdo.png"));
    disc(shader_for(BodyKind::Verdalia).as_ref(), &uniforms(l, 40.0, 0.2), &format!("{OUT}/shader_verdalia_detail02.png"));

    // Anillo: disco plano visto de frente, uv.x = radio normalizado.
    let n = 256;
    let mut img = vec![0u8; n * n * 3];
    let ring = ring_u();
    for y in 0..n {
        for x in 0..n {
            let sx = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let sy = 1.0 - (y as f32 + 0.5) / n as f32 * 2.0;
            let r = (sx * sx + sy * sy).sqrt();
            let c = if (0.45..=1.0).contains(&r) {
                let f = FragIn {
                    p_local: Vec3::new(sx * 1.31, 0.0, sy * 1.31),
                    normal: Vec3::new(0.0, 0.0, 1.0),
                    uv: [(r - 0.45) / 0.55, 0.5],
                    color: [1.0; 3],
                    view_dir: Vec3::Z,
                    distance: 5.0,
                };
                RingShader.shade(&f, &ring)
            } else {
                sky_color(Vec3::new(sx, sy, -1.0), 0.0)
            };
            img[(y * n + x) * 3..][..3].copy_from_slice(&to_byte(c));
        }
    }
    write_png(&format!("{OUT}/shader_anillo.png"), n, n, &img);

    disc(&ShipShader, &uniforms(l, 0.0, 1.0), &format!("{OUT}/shader_nave.png"));

    // Cielo: 512x256 equirectangular.
    let (w, h) = (512, 256);
    let mut img = vec![0u8; w * h * 3];
    for y in 0..h {
        for x in 0..w {
            let a = x as f32 / w as f32 * std::f32::consts::TAU;
            let b = (0.5 - y as f32 / h as f32) * std::f32::consts::PI;
            let d = Vec3::new(a.cos() * b.cos(), b.sin(), a.sin() * b.cos());
            img[(y * w + x) * 3..][..3].copy_from_slice(&to_byte(sky_color(d, 0.0)));
        }
    }
    write_png(&format!("{OUT}/shader_cielo.png"), w, h, &img);
}

fn ring_u() -> Uniforms {
    uniforms(Vec3::new(0.0, 0.3, 1.0), 0.0, 1.0)
}
