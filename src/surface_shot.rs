//! `--surface-shot out.png --body verdalia --alt 40 [--seed N] [--time T] [--yaw DEG] [--pitch DEG]`
//!
//! Captura sin ventana con `Scene::render` real: pone la nave a `alt` metros sobre el suelo del cuerpo (en el sitio
//! con más objetos, buscado con la semilla), mirando al horizonte, y deja que el terreno y los objetos se generen.
use crate::app::{FOV_Y, HEIGHT, Scene, WIDTH};
use crate::math::{DVec3, Quat, Vec3};
use crate::raster::{Framebuffer, RenderOptions, Stats};
use crate::scatter::{Models, Scatter};
use crate::ship::{CameraRig, Sim};
use crate::world::{BodyKind, solar_system};
use std::path::Path;
use std::sync::Arc;

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

pub fn run(out: &Path, args: &[String]) {
    let name = arg(args, "--body").unwrap_or_else(|| "verdalia".into()).to_lowercase();
    let alt: f64 = arg(args, "--alt").and_then(|a| a.parse().ok()).unwrap_or(40.0);
    let time: f64 = arg(args, "--time").and_then(|a| a.parse().ok()).unwrap_or(0.0);
    let yaw: f32 = arg(args, "--yaw").and_then(|a| a.parse().ok()).unwrap_or(0.0);
    let pitch: f32 = arg(args, "--pitch").and_then(|a| a.parse().ok()).unwrap_or(-14.0);
    let kind = match name.as_str() {
        "fornax" => BodyKind::Fornax,
        "nix" => BodyKind::Nix,
        "mutante" => BodyKind::Mutante,
        "siroco" => BodyKind::Siroco,
        _ => BodyKind::Verdalia,
    };
    let bodies = solar_system();
    let body = bodies.iter().find(|b| b.kind == kind).unwrap().clone();

    // Sitio con más objetos (misma semilla que usará la escena).
    let models = Arc::new(Models::load("assets/models").expect("assets/models"));
    let sunward = body.world_to_fixed(time, body.position(time) * -1.0).normalized();
    let spot = Scatter::new(kind, body.radius, crate::scatter::global_seed(), models)
        .map(|scatter| scatter.find_dense_spot_facing(2000, Some(sunward)))
        .unwrap_or(sunward);
    let r = crate::world::surface_height(&body, spot);
    let local = spot * (r + alt);
    let mut sim = Sim::new(bodies, body.position(time) + body.fixed_to_world(time, local));
    sim.t = time;
    // La nave va solidaria con el suelo (como en el juego): sin esto la predicción de `Terrain::warm` busca chunks
    // donde el planeta habrá girado y la base del terreno nunca queda lista.
    let rel = sim.ship.position - body.position(time);
    sim.ship.velocity = body.velocity(time) + body.spin_velocity(rel);
    let up_w = body.fixed_to_world(time, spot).to_f32().normalized();
    // Orientación: +Y de la nave = vertical local; -Z = una tangente cualquiera (más el giro pedido).
    let axis = Vec3::Y.cross(up_w);
    let ang = Vec3::Y.dot(up_w).clamp(-1.0, 1.0).acos();
    let q = if axis.length() < 1e-6 { Quat::IDENTITY } else { Quat::from_axis_angle(axis, ang) };
    sim.ship.orientation = (Quat::from_axis_angle(up_w, yaw.to_radians()) * q).normalized();
    let mut cam = CameraRig::new();
    cam.snap(&sim.ship);
    cam.offset = Vec3::new(0.0, 0.0, -40.0); // ojo 40 m delante de la nave: el casco queda detrás de la cámara
    cam.look_pitch = pitch.to_radians();

    let mut scene = Scene::new();
    // `--scale S` (p. ej. 0.7) renderiza a esa fracción de 960x540, como la resolución dinámica del juego.
    let scale: f32 = arg(args, "--scale").and_then(|a| a.parse().ok()).unwrap_or(1.0);
    let mut fb = Framebuffer::new(((WIDTH as f32 * scale).round() as usize).max(1), ((HEIGHT as f32 * scale).round() as usize).max(1));
    let opts = RenderOptions::default();
    let mut stats = Stats::default();
    // Varios cuadros: el terreno y las mallas de objetos se generan con presupuesto por cuadro.
    for _ in 0..40 {
        stats = Stats::default();
        scene.render(&mut fb, &sim, &cam, &opts, true, &mut stats);
    }
    // `--bench-ground`: mide 90 cuadros más en esa misma vista (fluidez a ras de suelo, sin ventana).
    if args.iter().any(|a| a == "--bench-ground") {
        let (mut sum, mut max, mut over) = (0.0f64, 0.0f64, 0u32);
        let mut st = [0.0f32; 5];
        for _ in 0..90 {
            stats = Stats::default();
            let t0 = std::time::Instant::now();
            scene.render(&mut fb, &sim, &cam, &opts, true, &mut stats);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            sum += ms;
            max = max.max(ms);
            if ms > 16.7 { over += 1; }
            st[0] += stats.ms_sky; st[1] += stats.ms_vertex; st[2] += stats.ms_raster; st[3] += stats.ms_shade; st[4] += stats.ms_atmosphere;
        }
        println!(
            "{} alt {alt} m: {:.2} ms medio, {:.2} ms máx, {over}/90 sobre 16.7 ms | cielo {:.2} vértices {:.2} raster {:.2} sombreado {:.2} atmósfera {:.2} | terreno {} tris, dibujados {}",
            body.name, sum / 90.0, max, st[0] / 90.0, st[1] / 90.0, st[2] / 90.0, st[3] / 90.0, st[4] / 90.0, scene.terrain_triangles, stats.triangles_drawn
        );
    }
    let s = &scene.scatter_stats;
    println!(
        "{} alt {alt} m semilla {}: terreno {} tris | objetos: total {} mostrados {} (completos {}, simples {}) descartados por distancia {} | {} tris objetos (sin LOD {}) en {} draw calls | triángulos dibujados {}",
        body.name, crate::scatter::global_seed(), scene.terrain_triangles, s.instances_total, s.instances_shown(), s.instances_high, s.instances_low,
        s.instances_culled_distance, s.triangles, s.triangles_without_lod, s.draw_calls, stats.triangles_drawn
    );
    let _ = (FOV_Y, DVec3::ZERO);
    fb.save_png(out).expect("no se pudo guardar el PNG");
}
