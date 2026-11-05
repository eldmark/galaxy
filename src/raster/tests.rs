use super::*;
use crate::geometry::Vertex;
use std::collections::HashMap;

const SCRATCH: &str = "/tmp/claude-1000/-home-dmark123-Documents-homework-graficas-galaxy/146d9493-3f05-4f8b-853c-4441b3071a9a/scratchpad";

struct Flat(pub [f32; 3]);
impl FragmentShader for Flat {
    fn shade(&self, _f: &FragIn, _u: &Uniforms) -> [f32; 3] {
        self.0
    }
}
/// Lambert simple con color de vértice.
struct Lambert;
impl FragmentShader for Lambert {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3] {
        let d = f.normal.dot(u.light_dir).max(0.0) * 0.85 + 0.15;
        [f.color[0] * d, f.color[1] * d, f.color[2] * d]
    }
}

fn uniforms() -> Uniforms {
    Uniforms { light_dir: Vec3::new(0.5, 0.8, 0.6).normalized(), time: 0.0, detail: 1.0, model: Mat4::IDENTITY }
}

fn mk_mesh(positions: &[Vec3], indices: &[u32]) -> Mesh {
    let vertices = positions
        .iter()
        .map(|&p| Vertex { position: p, normal: Vec3::Z, uv: [0.0, 0.0], color: [1.0; 3] })
        .collect();
    let mut m = Mesh { vertices, indices: indices.to_vec(), bounds_center: Vec3::ZERO, bounds_radius: 0.0 };
    set_bounds(&mut m);
    m
}

fn set_bounds(m: &mut Mesh) {
    let mut c = Vec3::ZERO;
    for v in &m.vertices {
        c += v.position;
    }
    c = c * (1.0 / m.vertices.len().max(1) as f32);
    let r = m.vertices.iter().map(|v| (v.position - c).length()).fold(0.0, f32::max);
    m.bounds_center = c;
    m.bounds_radius = r;
}

fn icosphere(sub: u32) -> Mesh {
    let t = (1.0 + 5f32.sqrt()) / 2.0;
    let mut p: Vec<Vec3> = [
        (-1.0, t, 0.0), (1.0, t, 0.0), (-1.0, -t, 0.0), (1.0, -t, 0.0),
        (0.0, -1.0, t), (0.0, 1.0, t), (0.0, -1.0, -t), (0.0, 1.0, -t),
        (t, 0.0, -1.0), (t, 0.0, 1.0), (-t, 0.0, -1.0), (-t, 0.0, 1.0),
    ]
    .iter()
    .map(|&(x, y, z)| Vec3::new(x, y, z).normalized())
    .collect();
    let mut f: Vec<[u32; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    for _ in 0..sub {
        let mut cache: HashMap<(u32, u32), u32> = HashMap::new();
        let mut mid = |a: u32, b: u32, p: &mut Vec<Vec3>| -> u32 {
            let k = (a.min(b), a.max(b));
            *cache.entry(k).or_insert_with(|| {
                p.push(((p[a as usize] + p[b as usize]) * 0.5).normalized());
                p.len() as u32 - 1
            })
        };
        let mut nf = Vec::new();
        for [a, b, c] in f {
            let (ab, bc, ca) = (mid(a, b, &mut p), mid(b, c, &mut p), mid(c, a, &mut p));
            nf.extend_from_slice(&[[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]]);
        }
        f = nf;
    }
    let vertices = p
        .iter()
        .map(|&q| Vertex { position: q, normal: q, uv: [0.0; 2], color: [1.0; 3] })
        .collect();
    let mut m = Mesh { vertices, indices: f.concat(), bounds_center: Vec3::ZERO, bounds_radius: 1.0 };
    set_bounds(&mut m);
    m
}

fn cube() -> Mesh {
    let mut verts = Vec::new();
    let mut idx = Vec::new();
    // (normal, u, v) con u x v = normal -> antihorario visto desde fuera
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Z, Vec3::Y),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::X, Vec3::Z),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::Y, Vec3::X),
    ];
    for (n, u, v) in faces {
        let b = verts.len() as u32;
        for (a, c) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            verts.push(Vertex {
                position: n + u * a + v * c,
                normal: n,
                uv: [(a + 1.0) / 2.0, (c + 1.0) / 2.0],
                color: [1.0; 3],
            });
        }
        idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
    }
    let mut m = Mesh { vertices: verts, indices: idx, bounds_center: Vec3::ZERO, bounds_radius: 0.0 };
    set_bounds(&mut m);
    m
}

fn call<'a>(mesh: &'a Mesh, model: Mat4, shader: &'a dyn FragmentShader) -> DrawCall<'a> {
    DrawCall { mesh, model, shader, uniforms: uniforms() }
}

fn px(fb: &Framebuffer, x: usize, y: usize) -> [u8; 4] {
    let i = (y * fb.width + x) * 4;
    [fb.color[i], fb.color[i + 1], fb.color[i + 2], fb.color[i + 3]]
}

fn persp() -> Mat4 {
    Mat4::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.5, 100.0)
}

fn black_fb(w: usize, h: usize) -> Framebuffer {
    let mut fb = Framebuffer::new(w, h);
    fb.clear([0, 0, 0]);
    fb
}

#[test]
fn known_triangle_covers_expected_pixels() {
    // proj = identidad: coordenadas de clip = NDC (w = 1).
    let m = mk_mesh(&[Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, -1.0, 0.0), Vec3::new(-1.0, 1.0, 0.0)], &[0, 1, 2]);
    let sh = Flat([1.0, 0.0, 0.0]);
    let mut fb = black_fb(8, 8);
    let mut st = Stats::default();
    render_scene(&mut fb, Mat4::IDENTITY, Mat4::IDENTITY, &[call(&m, Mat4::IDENTITY, &sh)], &RenderOptions::default(), &mut st);
    let mut n = 0;
    for y in 0..8 {
        for x in 0..8 {
            let on = px(&fb, x, y)[0] == 255;
            // centro del píxel: x+y(ndc) = (px-py)/4; la hipotenusa (derecha) se excluye.
            assert_eq!(on, x < y, "pixel ({x},{y})");
            n += on as usize;
        }
    }
    assert_eq!(n, 28);
    assert_eq!(st.triangles_drawn, 1);
    assert_eq!(st.pixels_shaded, 28);
}

#[test]
fn shared_edges_leave_no_holes_and_no_double_hits() {
    // Cuadrado completo con diagonal, y abanico de 8 triángulos con vértice interior.
    let sh = Flat([0.0, 1.0, 0.0]);
    let quad = mk_mesh(
        &[Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(-1.0, 1.0, 0.0)],
        &[0, 1, 2, 0, 2, 3],
    );
    let ring = [(-1.0, -1.0), (0.0, -1.0), (1.0, -1.0), (1.0, 0.1), (1.0, 1.0), (-0.3, 1.0), (-1.0, 1.0), (-1.0, 0.2)];
    let mut pos: Vec<Vec3> = ring.iter().map(|&(x, y)| Vec3::new(x, y, 0.0)).collect();
    pos.push(Vec3::new(0.137, -0.291, 0.0));
    let idx: Vec<u32> = (0..8u32).flat_map(|i| [8, i, (i + 1) % 8]).collect();
    let fan = mk_mesh(&pos, &idx);
    for (name, m) in [("quad", &quad), ("fan", &fan)] {
        for (w, h) in [(8usize, 8usize), (131, 77)] {
            let mut fb = black_fb(w, h);
            let mut st = Stats::default();
            let c = call(m, Mat4::IDENTITY, &sh);
            let info = pipeline::render(&mut fb, Mat4::IDENTITY, Mat4::IDENTITY, &[c], &RenderOptions::default(), &mut st);
            assert_eq!(info.raster_hits, (w * h) as u64, "{name} {w}x{h}: hits = pixeles");
            assert_eq!(st.pixels_shaded, (w * h) as u64, "{name} {w}x{h}: sin huecos");
        }
    }
}

#[test]
fn zbuffer_picks_nearest_regardless_of_order() {
    let near = mk_mesh(&[Vec3::new(-1.0, -1.0, -2.0), Vec3::new(1.0, -1.0, -2.0), Vec3::new(0.0, 1.0, -2.0)], &[0, 1, 2]);
    let far = mk_mesh(&[Vec3::new(-3.0, -3.0, -4.0), Vec3::new(3.0, -3.0, -4.0), Vec3::new(0.0, 3.0, -4.0)], &[0, 1, 2]);
    let (red, blue) = (Flat([1.0, 0.0, 0.0]), Flat([0.0, 0.0, 1.0]));
    let mut results = Vec::new();
    for order in 0..2 {
        let mut fb = black_fb(32, 32);
        let mut st = Stats::default();
        let a = call(&near, Mat4::IDENTITY, &red);
        let b = call(&far, Mat4::IDENTITY, &blue);
        let calls = if order == 0 { [a, b] } else { [b, a] };
        render_scene(&mut fb, Mat4::IDENTITY, persp(), &calls, &RenderOptions::default(), &mut st);
        assert_eq!(px(&fb, 16, 16), [255, 0, 0, 255]);
        assert_eq!(px(&fb, 16, 5), [0, 0, 255, 255]);
        results.push(fb.color.clone());
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn near_plane_clipping_is_finite() {
    // Un vértice detrás de la cámara (z = +1), dos delante.
    let m = mk_mesh(&[Vec3::new(0.0, -1.0, 1.0), Vec3::new(2.0, -1.0, -3.0), Vec3::new(-2.0, -1.0, -3.0)], &[0, 2, 1]);
    let sh = Flat([1.0, 1.0, 1.0]);
    let mut fb = black_fb(64, 64);
    let mut st = Stats::default();
    let opts = RenderOptions { frustum_culling: true, backface_culling: false };
    render_scene(&mut fb, Mat4::IDENTITY, persp(), &[call(&m, Mat4::IDENTITY, &sh)], &opts, &mut st);
    assert_eq!(st.triangles_in, 1);
    assert_eq!(st.triangles_drawn, 1);
    assert!(st.pixels_shaded > 0);
    assert!(fb.depth.iter().all(|d| d.is_finite() && *d >= 0.0));
    // Totalmente detrás: descartado.
    let behind = mk_mesh(&[Vec3::new(0.0, 0.0, 2.0), Vec3::new(1.0, 0.0, 2.0), Vec3::new(0.0, 1.0, 2.0)], &[0, 1, 2]);
    let mut st2 = Stats::default();
    let opts = RenderOptions { frustum_culling: false, backface_culling: false };
    render_scene(&mut fb, Mat4::IDENTITY, persp(), &[call(&behind, Mat4::IDENTITY, &sh)], &opts, &mut st2);
    assert_eq!(st2.triangles_clipped_out, 1);
    assert_eq!(st2.triangles_drawn, 0);
}

#[test]
fn clipping_keeps_attributes_correct() {
    // Triángulo que cruza near; el color de vértice interpolado debe quedar en 0..1 sin NaN.
    struct ColorOut;
    impl FragmentShader for ColorOut {
        fn shade(&self, f: &FragIn, _u: &Uniforms) -> [f32; 3] {
            assert!(f.color.iter().all(|c| c.is_finite() && *c >= -1e-4 && *c <= 1.0001), "{:?}", f.color);
            assert!(f.distance.is_finite() && f.view_dir.length() > 0.99);
            f.color
        }
    }
    let mut m = mk_mesh(&[Vec3::new(0.0, -1.0, 1.0), Vec3::new(2.0, -1.0, -3.0), Vec3::new(-2.0, -1.0, -3.0)], &[0, 2, 1]);
    m.vertices[0].color = [1.0, 0.0, 0.0];
    m.vertices[1].color = [0.0, 1.0, 0.0];
    m.vertices[2].color = [0.0, 0.0, 1.0];
    let mut fb = black_fb(64, 64);
    let mut st = Stats::default();
    let opts = RenderOptions { frustum_culling: false, backface_culling: false };
    render_scene(&mut fb, Mat4::IDENTITY, persp(), &[call(&m, Mat4::IDENTITY, &ColorOut)], &opts, &mut st);
    assert!(st.pixels_shaded > 0);
}

#[test]
fn frustum_culling_discards_and_counts() {
    let s = icosphere(2);
    let sh = Flat([1.0, 1.0, 1.0]);
    let view = Mat4::IDENTITY;
    let behind = Mat4::translation(Vec3::new(0.0, 0.0, 50.0));
    let ahead = Mat4::translation(Vec3::new(0.0, 0.0, -10.0));
    let side = Mat4::translation(Vec3::new(500.0, 0.0, -10.0));
    let too_far = Mat4::translation(Vec3::new(0.0, 0.0, -1000.0));
    let calls = [call(&s, behind, &sh), call(&s, ahead, &sh), call(&s, side, &sh), call(&s, too_far, &sh)];
    let mut fb = black_fb(64, 64);
    let mut st = Stats::default();
    render_scene(&mut fb, view, persp(), &calls, &RenderOptions::default(), &mut st);
    assert_eq!(st.draw_calls_total, 4);
    assert_eq!(st.draw_calls_culled, 3);
    assert_eq!(st.triangles_in, s.triangle_count() as u64);

    let mut st2 = Stats::default();
    let opts = RenderOptions { frustum_culling: false, backface_culling: true };
    render_scene(&mut fb, view, persp(), &calls, &opts, &mut st2);
    assert_eq!(st2.draw_calls_culled, 0);
    assert_eq!(st2.triangles_in, 4 * s.triangle_count() as u64);
    assert!(st2.triangles_in > st.triangles_in);
    assert_eq!(st2.triangles_in, st2.triangles_backface + st2.triangles_clipped_out + st2.triangles_drawn);
}

#[test]
fn culling_uses_largest_scale() {
    // Esfera pequeña (radio 1) escalada x100 en X: aún visible aunque su centro esté fuera.
    let s = icosphere(1);
    let sh = Flat([1.0; 3]);
    let model = Mat4::translation(Vec3::new(30.0, 0.0, -10.0)) * Mat4::scale(Vec3::new(100.0, 1.0, 1.0));
    let mut fb = black_fb(32, 32);
    let mut st = Stats::default();
    render_scene(&mut fb, Mat4::IDENTITY, persp(), &[call(&s, model, &sh)], &RenderOptions::default(), &mut st);
    assert_eq!(st.draw_calls_culled, 0);
}

#[test]
fn backface_culling_on_cube() {
    let c = cube();
    let sh = Flat([1.0; 3]);
    let model = Mat4::translation(Vec3::new(0.0, 0.0, -5.0)) * Mat4::rotation_y(0.5);
    let mut fb = black_fb(64, 64);
    let mut st = Stats::default();
    render_scene(&mut fb, Mat4::IDENTITY, persp(), &[call(&c, model, &sh)], &RenderOptions::default(), &mut st);
    assert!(st.triangles_backface >= 6 && st.triangles_backface <= 8, "{:?}", st);
    let mut fb2 = black_fb(64, 64);
    let mut st2 = Stats::default();
    let opts = RenderOptions { frustum_culling: true, backface_culling: false };
    render_scene(&mut fb2, Mat4::IDENTITY, persp(), &[call(&c, model, &sh)], &opts, &mut st2);
    assert_eq!(st2.triangles_backface, 0);
    assert!(st2.triangles_drawn > st.triangles_drawn);
    assert_eq!(fb.color, fb2.color, "la imagen no cambia: las caras traseras quedan tapadas");
    // Espejo (escala negativa): la orientación se corrige, el cubo sigue viéndose.
    let mut fb3 = black_fb(64, 64);
    let mut st3 = Stats::default();
    let mirrored = model * Mat4::scale(Vec3::new(-1.0, 1.0, 1.0));
    render_scene(&mut fb3, Mat4::IDENTITY, persp(), &[call(&c, mirrored, &sh)], &RenderOptions::default(), &mut st3);
    assert_eq!(st3.triangles_drawn, st.triangles_drawn);
}

fn scene_frame(w: usize, h: usize) -> (Framebuffer, Stats) {
    let s = icosphere(3);
    let c = cube();
    let lam = Lambert;
    let model_a = Mat4::translation(Vec3::new(-1.2, 0.0, -5.0)) * Mat4::rotation_y(0.4);
    let model_b = Mat4::translation(Vec3::new(1.5, -0.3, -4.0)) * Mat4::rotation_y(0.6) * Mat4::rotation_x(0.4) * Mat4::uniform_scale(0.7);
    let ground = Mat4::translation(Vec3::new(0.0, -1.5, -5.0)) * Mat4::scale(Vec3::new(10.0, 0.05, 10.0));
    let calls = [call(&s, model_a, &lam), call(&c, model_b, &lam), call(&c, ground, &lam)];
    let mut fb = Framebuffer::new(w, h);
    fb.clear([10, 10, 30]);
    let mut st = Stats::default();
    let proj = Mat4::perspective(1.0, w as f32 / h as f32, 0.5, 500.0);
    render_scene(&mut fb, Mat4::look_at(Vec3::new(0.0, 0.8, 1.0), Vec3::new(0.0, -0.3, -5.0), Vec3::Y), proj, &calls, &RenderOptions::default(), &mut st);
    (fb, st)
}

#[test]
fn deterministic_and_parallelism_independent() {
    let (a, sa) = scene_frame(320, 200);
    let (b, _) = scene_frame(320, 200);
    assert_eq!(a.color, b.color);
    assert_eq!(a.depth, b.depth);
    let pool = rayon::ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    let (c, sc) = pool.install(|| scene_frame(320, 200));
    assert_eq!(a.color, c.color);
    assert_eq!(a.depth, c.depth);
    assert_eq!(sa.triangles_drawn, sc.triangles_drawn);
    assert_eq!(sa.pixels_shaded, sc.pixels_shaded);
}

#[test]
fn render_to_png() {
    let (fb, st) = scene_frame(640, 400);
    println!("{st:?}");
    assert!(st.pixels_shaded > 10_000);
    let dir = std::path::Path::new(SCRATCH);
    std::fs::create_dir_all(dir).unwrap();
    let p = dir.join("raster_scene.png");
    fb.save_png(&p).unwrap();
    assert!(std::fs::metadata(&p).unwrap().len() > 1000);
}

#[test]
fn huge_planet_model_renders() {
    // Planeta de 4000 m de radio, cámara cerca de la superficie (near 0.5, far 500000).
    let s = icosphere(4);
    let lam = Lambert;
    let model = Mat4::translation(Vec3::new(0.0, -4000.0, -300.0)) * Mat4::uniform_scale(4000.0);
    let proj = Mat4::perspective(1.0, 1.6, 0.5, 500_000.0);
    let mut fb = black_fb(320, 200);
    let mut st = Stats::default();
    render_scene(&mut fb, Mat4::IDENTITY, proj, &[call(&s, model, &lam)], &RenderOptions::default(), &mut st);
    assert!(st.pixels_shaded > 5_000, "{st:?}");
    assert!(fb.depth.iter().all(|d| d.is_finite()));
}

#[test]
#[ignore]
fn bench_50k_triangles_960x540() {
    let s = icosphere(5); // 20480 triángulos
    let lam = Lambert;
    let models = [
        Mat4::translation(Vec3::new(-2.5, 0.0, -6.0)),
        Mat4::translation(Vec3::new(0.0, 0.0, -5.0)),
        Mat4::translation(Vec3::new(2.5, 0.0, -6.0)),
    ];
    let calls: Vec<DrawCall> = models.iter().map(|&m| call(&s, m, &lam)).collect();
    let mut fb = Framebuffer::new(960, 540);
    let proj = Mat4::perspective(1.0, 960.0 / 540.0, 0.5, 500.0);
    let mut total = 0.0;
    let n = 60;
    let mut last = Stats::default();
    for i in 0..n + 5 {
        fb.clear([0, 0, 0]);
        let mut st = Stats::default();
        let t = std::time::Instant::now();
        render_scene(&mut fb, Mat4::IDENTITY, proj, &calls, &RenderOptions::default(), &mut st);
        if i >= 5 {
            total += t.elapsed().as_secs_f64() * 1000.0;
        }
        last = st;
    }
    let avg = total / n as f64;
    println!("threads={} tris_in={} avg {:.2} ms ({:.0} FPS) {:?}", rayon::current_num_threads(), last.triangles_in, avg, 1000.0 / avg, last);
}
