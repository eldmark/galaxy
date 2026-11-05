//! Demostración sin ventana: `--terrain-shot out.png --alt 300 --kind verdalia`.
//! Dibuja el terreno con el pipeline real (`render_scene`, `shader_for`, cielo) desde una altitud dada.
//!
//! OJO: `demo_height` es una altura RUIDOSA TEMPORAL solo para esta demostración (mientras `relief::height`
//! devuelve 0). Cuando `relief` esté integrada se cambia `demo_height` por `relief::height(kind, dir)`
//! (basta pasar `Terrain::new` en lugar de `Terrain::with_height`).
use super::{Frustum, Terrain};
use crate::math::{Mat4, Vec3};
use crate::raster::{DrawCall, Framebuffer, RenderOptions, Stats, Uniforms, render_scene};
use crate::shading::{shader_for, sky_color};
use crate::world::{BodyKind, solar_system};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

const W: usize = 960;
const H: usize = 540;
const FOV_Y: f32 = 1.0;
/// Amplitud relativa de la altura de demostración (+-2 % del radio = +-70 m en Verdalia).
const DEMO_AMP: f32 = 0.02;

fn hash3(x: i32, y: i32, z: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ (z as u32).wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 16;
    (h & 0xffff) as f32 / 65535.0
}

fn vnoise(p: Vec3) -> f32 {
    let (ix, iy, iz) = (p.x.floor(), p.y.floor(), p.z.floor());
    let (fx, fy, fz) = (p.x - ix, p.y - iy, p.z - iz);
    let s = |t: f32| t * t * (3.0 - 2.0 * t);
    let (sx, sy, sz) = (s(fx), s(fy), s(fz));
    let (ix, iy, iz) = (ix as i32, iy as i32, iz as i32);
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    // interpolación trilineal explícita
    let v = |dx: i32, dy: i32, dz: i32| hash3(ix + dx, iy + dy, iz + dz);
    let x00 = l(v(0, 0, 0), v(1, 0, 0), sx);
    let x10 = l(v(0, 1, 0), v(1, 1, 0), sx);
    let x01 = l(v(0, 0, 1), v(1, 0, 1), sx);
    let x11 = l(v(0, 1, 1), v(1, 1, 1), sx);
    l(l(x00, x10, sy), l(x01, x11, sy), sz)
}

/// Altura temporal de demostración: fbm de crestas, en `[-DEMO_AMP, DEMO_AMP]`.
pub fn demo_height(d: Vec3) -> f32 {
    let (mut sum, mut amp, mut freq, mut norm) = (0.0f32, 1.0f32, 3.0f32, 0.0f32);
    for o in 0..9 {
        let n = vnoise(d * freq + Vec3::new(7.3 * o as f32, 1.7, 3.1));
        let ridge = 1.0 - (2.0 * n - 1.0).abs();
        sum += ridge * ridge * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    (sum / norm * 2.0 - 0.8) * DEMO_AMP
}

fn smooth01(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Cielo: estrellas en el espacio, azul de atmósfera cerca del suelo.
fn paint_sky(fb: &mut Framebuffer, view: Mat4, up: Vec3, altitude: f32) {
    let t = (FOV_Y * 0.5).tan();
    let aspect = fb.width as f32 / fb.height as f32;
    let air = 1.0 - smooth01(500.0, 9000.0, altitude);
    for y in 0..fb.height {
        for x in 0..fb.width {
            let nx = (x as f32 + 0.5) / fb.width as f32 * 2.0 - 1.0;
            let ny = 1.0 - (y as f32 + 0.5) / fb.height as f32 * 2.0;
            let d = Vec3::new(nx * aspect * t, ny * t, -1.0).normalized();
            let w = view.transpose().transform_dir(d);
            let mut c = if air < 0.999 { sky_color(w, 0.0) } else { [0.0; 3] };
            if air > 0.0 {
                let e = w.dot(up).max(0.0).sqrt();
                let blue = [0.62 + (0.14 - 0.62) * e, 0.78 + (0.34 - 0.78) * e, 0.95 + (0.72 - 0.95) * e];
                for k in 0..3 {
                    c[k] = c[k] * (1.0 - air) + blue[k] * air;
                }
            }
            let i = (y * fb.width + x) * 4;
            for k in 0..3 {
                fb.color[i + k] = (c[k].clamp(0.0, 1.0) * 255.0) as u8;
            }
            fb.color[i + 3] = 255;
        }
    }
}

/// Captura del terreno de `kind` desde `altitude_m` metros sobre el suelo, mirando al horizonte.
pub fn shot(out: &Path, altitude_m: f32, kind: BodyKind) -> std::io::Result<()> {
    let body = solar_system().into_iter().find(|b| b.kind == kind).expect("cuerpo desconocido");
    let radius = body.radius;
    let mut terrain = Terrain::with_height(kind, radius, 8, DEMO_AMP as f64 * 1.01, Arc::new(demo_height));
    terrain.budget = usize::MAX; // captura estática: todo listo en un cuadro

    let dir0 = Vec3::new(0.3, 0.8, 0.52).normalized();
    let ground = radius * (1.0 + demo_height(dir0) as f64);
    let cam = dir0.to_f64() * (ground + altitude_m as f64);
    let east = Vec3::Y.cross(dir0).normalized();
    let dip = (radius / (radius + altitude_m as f64)).acos() as f32;
    // cerca del suelo el horizonte queda en el tercio superior; desde lejos se mira más hacia abajo para ver el suelo
    let pitch = -if dip < 0.3 { 0.6 } else { 0.9 } * dip;
    let fwd = east * pitch.cos() + dir0 * pitch.sin();
    let cam_f = cam.to_f32();
    let view = Mat4::look_at(cam_f, cam_f + fwd, dir0);
    let near = (altitude_m * 0.03).clamp(0.2, 60.0);
    let proj = Mat4::perspective(FOV_Y, W as f32 / H as f32, near, 400_000.0);
    let fr = Frustum::from_camera(cam, fwd.to_f64(), dir0.to_f64(), FOV_Y as f64, W as f64 / H as f64);

    let t0 = Instant::now();
    let chunks = terrain.frame(cam, FOV_Y as f64, H as f64, Some(&fr));
    let ms_frame = t0.elapsed().as_secs_f32() * 1000.0;
    let (ms_sel, ms_gen) = (terrain.stats.ms_select, terrain.stats.ms_generate);
    // segundo cuadro con la caché caliente (lo que verá el bucle principal)
    let t1 = Instant::now();
    let _ = terrain.frame(cam, FOV_Y as f64, H as f64, Some(&fr));
    let ms_warm = t1.elapsed().as_secs_f32() * 1000.0;

    let shader = shader_for(kind);
    let light = (dir0 * 0.55 + east * 0.8 + dir0.cross(east) * 0.25).normalized();
    let mut fb = Framebuffer::new(W, H);
    paint_sky(&mut fb, view, dir0, altitude_m);
    let model = Mat4::IDENTITY;
    let calls: Vec<DrawCall> = chunks
        .iter()
        .map(|(_, m)| DrawCall {
            mesh: m,
            model,
            shader: shader.as_ref(),
            uniforms: Uniforms { light_dir: light, time: 5.0, detail: 1.0, model },
        })
        .collect();
    let mut stats = Stats::default();
    let t2 = Instant::now();
    render_scene(&mut fb, view, proj, &calls, &RenderOptions::default(), &mut stats);
    let ms_render = t2.elapsed().as_secs_f32() * 1000.0;
    fb.save_png(out)?;
    let s = &terrain.stats;
    println!(
        "alt {altitude_m:>6} m | selección {ms_sel:.2} ms + generación {ms_gen:.1} ms (cuadro frío {ms_frame:.1} ms, caliente {ms_warm:.2} ms) | render {ms_render:.1} ms\n  \
         visitados {} | horizonte {} | frustum {} | hojas {} (niveles {}..{}) | dibujados {} chunks, {} tris (raster: {} dibujados) | caché {}",
        s.visited, s.culled_horizon, s.culled_frustum, s.leaves, s.min_level, s.max_level, s.drawn, s.triangles, stats.triangles_drawn, s.cache_size
    );
    Ok(())
}

/// Nombre de línea de comandos -> cuerpo (por defecto Verdalia).
pub fn kind_from_name(n: &str) -> BodyKind {
    match n.to_lowercase().as_str() {
        "fornax" => BodyKind::Fornax,
        "mutante" => BodyKind::Mutante,
        "nix" => BodyKind::Nix,
        _ => BodyKind::Verdalia,
    }
}
