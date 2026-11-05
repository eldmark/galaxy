use super::*;
use crate::geometry::Mesh;
use crate::math::{Mat4, Vec3};

const KINDS: [BodyKind; 6] =
    [BodyKind::Sun, BodyKind::Verdalia, BodyKind::Siroco, BodyKind::Mutante, BodyKind::Nix, BodyKind::Fornax];

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 24) as f32
    }
    fn dir(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.next() * 2.0 - 1.0, self.next() * 2.0 - 1.0, self.next() * 2.0 - 1.0);
            let l = v.length();
            if l > 0.1 && l <= 1.0 {
                return v * (1.0 / l);
            }
        }
    }
}

#[test]
fn deterministic() {
    let mut r = Rng(7);
    for _ in 0..500 {
        let d = r.dir();
        for k in KINDS {
            assert_eq!(height(k, d), height(k, d));
        }
    }
}

#[test]
fn bounded_by_max_amplitude_and_uses_range() {
    let mut r = Rng(12345);
    for k in KINDS {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..20_000 {
            let h = height(k, r.dir());
            assert!(h.is_finite());
            assert!(h.abs() <= max_amplitude(k) + 1e-7, "{k:?} {h}");
            lo = lo.min(h);
            hi = hi.max(h);
        }
        let a = max_amplitude(k);
        println!("{k:?}: amp {a} rango [{lo:.4}, {hi:.4}]");
        if a > 0.0 {
            assert!(hi - lo > 0.8 * a, "{k:?} usa poco rango: {}", hi - lo);
        }
    }
}

#[test]
fn sun_and_siroco_flat() {
    let mut r = Rng(3);
    for _ in 0..1000 {
        let d = r.dir();
        assert_eq!(height(BodyKind::Sun, d), 0.0);
        assert_eq!(height(BodyKind::Siroco, d), 0.0);
    }
    assert_eq!(max_amplitude(BodyKind::Sun), 0.0);
    assert_eq!(max_amplitude(BodyKind::Siroco), 0.0);
}

#[test]
fn continuous_between_nearby_directions() {
    let mut r = Rng(99);
    for k in [BodyKind::Verdalia, BodyKind::Fornax, BodyKind::Nix] {
        let mut worst = 0.0f32;
        for _ in 0..5000 {
            let d = r.dir();
            let e = (r.dir() * 1e-3 + d).normalized();
            worst = worst.max((height(k, d) - height(k, e)).abs());
        }
        // ~1e-3 rad de separación: salto < 12 % de la amplitud
        println!("{k:?} peor salto a 1e-3 rad: {worst} (amp {})", max_amplitude(k));
        assert!(worst < 0.12 * max_amplitude(k), "{k:?} {worst}");
    }
}

#[test]
fn mutante_is_quantized_to_16_levels() {
    let mut r = Rng(5);
    let mut levels: Vec<i32> = Vec::new();
    for _ in 0..20_000 {
        let h = height(BodyKind::Mutante, r.dir()) / AMP_MUTANTE; // -1..1
        let q = (h + 1.0) * 0.5 * (MUTANTE_STEPS - 1) as f32;
        assert!((q - q.round()).abs() < 1e-3, "no cuantizado: {q}");
        let l = q.round() as i32;
        if !levels.contains(&l) {
            levels.push(l);
        }
    }
    assert!(levels.len() >= 10 && levels.len() <= 16, "{}", levels.len());
}

#[test]
fn verdalia_land_is_higher_than_sea_following_the_shader_threshold() {
    // Donde el ruido del shader dice "mar" (< 0.44) la altura es <= 0; la tierra queda más alta en media.
    let mut r = Rng(31);
    let (mut sea, mut land, mut n_sea, mut n_land) = (0.0f32, 0.0f32, 0, 0);
    for _ in 0..20_000 {
        let d = r.dir();
        let c = fbm(d, 3.5, Vec3::new(4.0, 0.0, 0.0), 3);
        let h = height(BodyKind::Verdalia, d);
        if c < 0.44 {
            assert!(h <= 1e-6);
            sea += h;
            n_sea += 1;
        } else if c > 0.54 {
            land += h;
            n_land += 1;
        }
    }
    assert!(n_sea > 100 && n_land > 100);
    assert!(land / n_land as f32 > 0.0 && sea / (n_sea as f32) < 0.0);
}

#[test]
fn displaced_lods_follow_height() {
    let l = displaced_lods(BodyKind::Verdalia, 3);
    assert_eq!(l.levels.len(), 4);
    for v in &l.levels[3].vertices {
        let d = v.position.normalized();
        let expect = 1.0 + height(BodyKind::Verdalia, d);
        assert!((v.position.length() - expect).abs() < 1e-5);
    }
    for v in &displaced_lods(BodyKind::Siroco, 2).levels[2].vertices {
        assert!((v.position.length() - 1.0).abs() < 1e-5);
    }
}

/// Error EXTRA de silueta que añade el relieve: diferencia entre la altura real en el centro de cada
/// triángulo y la que da la malla (interpolación de los 3 vértices), en píxeles a la distancia
/// donde la esfera lisa de ese nivel cae justo en MAX_ERROR_PX.
#[test]
fn silhouette_error_added_by_relief() {
    let (fov, h_px) = (1.0f32, 600.0f32);
    for k in [BodyKind::Verdalia, BodyKind::Fornax, BodyKind::Nix, BodyKind::Mutante] {
        let lods = displaced_lods(k, 5);
        for lvl in [3usize, 4, 5] {
            let ico = crate::geometry::icosphere(lvl as u32);
            let mut worst = 0.0f32;
            for t in ico.indices.chunks_exact(3) {
                let p: Vec<Vec3> = t.iter().map(|&i| ico.vertices[i as usize].position).collect();
                let mid = ((p[0] + p[1] + p[2]) * (1.0 / 3.0)).normalized();
                let real = height(k, mid);
                let interp = (height(k, p[0]) + height(k, p[1]) + height(k, p[2])) / 3.0;
                worst = worst.max((real - interp).abs());
            }
            let mut d = 1.2f32;
            while lods.error_px(lvl, d, 1.0, fov, h_px) > crate::geometry::lod::MAX_ERROR_PX && d < 1e4 {
                d *= 1.02;
            }
            let px = worst * h_px / (2.0 * d * (fov * 0.5).tan());
            println!("{k:?} nivel {lvl}: error de relieve max {worst:.5} R -> {px:.2} px a d={d:.2}R");
            // el modelo de LOD (SphereLods::relief_error_px) debe acotar lo medido
            assert!(worst <= max_amplitude(k) * (10.0 * lods.max_edge_angle[lvl]).min(2.0) * 1.05, "{k:?} {lvl}");
            let sel = lods.select_with_relief(d * 3.0, 1.0, fov, h_px, max_amplitude(k));
            assert!(sel >= lods.select(d * 3.0, 1.0, fov, h_px));
            let _ = px;
        }
    }
}

/// Imagen de comprobación con el pipeline real: solo si RELIEF_IMG_DIR está definida.
#[test]
fn render_check_images() {
    use crate::raster::{render_scene, DrawCall, Framebuffer, RenderOptions, Stats, Uniforms};
    let Ok(dir) = std::env::var("RELIEF_IMG_DIR") else { return };
    for k in [BodyKind::Verdalia, BodyKind::Fornax, BodyKind::Nix, BodyKind::Mutante, BodyKind::Siroco] {
        let lods = displaced_lods(k, 5);
        let mesh: &Mesh = &lods.levels[5];
        let shader = crate::shading::shader_for(k);
        let mut fb = Framebuffer::new(400, 300);
        fb.clear([4, 4, 10]);
        let eye = Vec3::new(0.0, 0.0, 1.9);
        let view = Mat4::look_at(eye, Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective(0.9, 400.0 / 300.0, 0.05, 50.0);
        let model = Mat4::rotation_y(0.9);
        let light = Vec3::new(1.0, 0.25, 0.1).normalized();
        let calls = [DrawCall {
            mesh,
            model,
            shader: shader.as_ref(),
            uniforms: Uniforms { light_dir: light, time: 3.0, detail: 1.0, model },
        }];
        let mut st = Stats::default();
        render_scene(&mut fb, view, proj, &calls, &RenderOptions::default(), &mut st);
        let path = std::path::Path::new(&dir).join(format!("relief_{:?}.png", k).to_lowercase());
        fb.save_png(&path).unwrap();
    }
}
