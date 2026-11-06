//! Bucle principal: une simulación, cámara, escena (LOD + culling), HUD y entrada.
use crate::geometry::{Mesh, SphereLods, ring_mesh};
use crate::shipmodel::ShipModel;
use crate::terrain::{ChunkKey, Frustum, Terrain};
use std::sync::Arc;
use crate::hud::{HudLayout, HudState, draw_hud_layout};
use crate::math::{DVec3, Mat4, Vec3};
use crate::platform::Platform;
use crate::raster::{DrawCall, Framebuffer, FragmentShader, RenderOptions, Stats, Uniforms, render_scene};
use crate::shading::{shader_for, shader_for_ring, ship_shader, sky_color};
use crate::ship::{CameraRig, Input, Sim};
use crate::world::{altitude, body_model_matrix, nearest_body, solar_system};
use rayon::prelude::*;
use std::path::Path;
use std::time::Instant;

pub const WIDTH: usize = 960;
pub const HEIGHT: usize = 540;
pub const FOV_Y: f32 = 1.0;
const NEAR: f32 = 1.0;
const FAR: f32 = 600_000.0;
const LOD_MARGIN: f32 = 0.25;
/// Altitud (en radios) a la que se cambia de esfera a terreno con chunks, con histéresis.
const TERRAIN_ON: f64 = 0.5;
const TERRAIN_OFF: f64 = 0.7;
const TERRAIN_MAX_LEVEL: u8 = 8;

/// Cielo precalculado como cubemap: las estrellas no cambian, así que `sky_color` se evalúa una vez
/// por texel al arrancar y cada cuadro solo muestrea (antes costaba ~27 ms por cuadro a 960x540).
pub struct SkyBox {
    size: usize,
    faces: Vec<Vec<[u8; 3]>>, // +X, -X, +Y, -Y, +Z, -Z
}

/// Dirección de un texel (sc, tc en [-1, 1]) de cada cara. Es la inversa exacta de `face_uv`.
fn face_dir(face: usize, sc: f32, tc: f32) -> Vec3 {
    match face {
        0 => Vec3::new(1.0, -tc, -sc),
        1 => Vec3::new(-1.0, -tc, sc),
        2 => Vec3::new(sc, 1.0, tc),
        3 => Vec3::new(sc, -1.0, -tc),
        4 => Vec3::new(sc, -tc, 1.0),
        _ => Vec3::new(-sc, -tc, -1.0),
    }
}

/// Cara y coordenadas (u, v) en [0, 1) de una dirección.
fn face_uv(d: Vec3) -> (usize, f32, f32) {
    let (ax, ay, az) = (d.x.abs(), d.y.abs(), d.z.abs());
    let (face, sc, tc, ma) = if ax >= ay && ax >= az {
        if d.x > 0.0 { (0, -d.z, -d.y, ax) } else { (1, d.z, -d.y, ax) }
    } else if ay >= az {
        if d.y > 0.0 { (2, d.x, d.z, ay) } else { (3, d.x, -d.z, ay) }
    } else if d.z > 0.0 {
        (4, d.x, -d.y, az)
    } else {
        (5, -d.x, -d.y, az)
    };
    (face, (sc / ma + 1.0) * 0.5, (tc / ma + 1.0) * 0.5)
}

impl SkyBox {
    pub fn new(size: usize, time: f32) -> Self {
        let faces = (0..6)
            .map(|f| {
                (0..size * size)
                    .into_par_iter()
                    .map(|i| {
                        let (x, y) = (i % size, i / size);
                        let sc = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                        let tc = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
                        let c = sky_color(face_dir(f, sc, tc).normalized(), time);
                        [0, 1, 2].map(|k| (c[k].clamp(0.0, 1.0) * 255.0) as u8)
                    })
                    .collect()
            })
            .collect();
        Self { size, faces }
    }

    pub fn sample(&self, d: Vec3) -> [u8; 3] {
        let (face, u, v) = face_uv(d);
        let last = self.size - 1;
        let x = ((u * self.size as f32) as usize).min(last);
        let y = ((v * self.size as f32) as usize).min(last);
        self.faces[face][y * self.size + x]
    }

    /// Pinta el fondo de todo el framebuffer (en paralelo por filas) y reinicia la profundidad.
    pub fn paint(&self, fb: &mut Framebuffer, view: Mat4) {
        let (w, h) = (fb.width, fb.height);
        let t = (FOV_Y * 0.5).tan();
        let aspect = w as f32 / h as f32;
        let inv = view.transpose();
        fb.color.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
            let ny = 1.0 - (y as f32 + 0.5) / h as f32 * 2.0;
            for x in 0..w {
                let nx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
                let d = inv.transform_dir(Vec3::new(nx * aspect * t, ny * t, -1.0));
                let c = self.sample(d);
                row[x * 4..x * 4 + 3].copy_from_slice(&c);
                row[x * 4 + 3] = 255;
            }
        });
        fb.depth.fill(0.0);
    }
}

pub fn new_sim() -> Sim {
    let bodies = solar_system();
    // Empieza frente a Verdalia, a ~4.5 radios, mirando hacia ella (la nave mira -Z).
    let v = bodies[2].position(0.0);
    let start = v + DVec3::new(0.0, 0.0, bodies[2].radius * 4.5);
    let vel = bodies[2].velocity(0.0);
    let mut sim = Sim::new(bodies, start);
    sim.ship.velocity = vel;
    sim
}

pub struct Scene {
    sky: SkyBox,
    shaders: Vec<Box<dyn FragmentShader>>,
    lods: Vec<SphereLods>,
    levels: Vec<usize>,
    ring: Mesh,
    ring_shader: Box<dyn FragmentShader>,
    ship: ShipModel,
    ship_shader: Box<dyn FragmentShader>,
    /// Terreno con LOD continuo por cuerpo aterrizable (reemplaza a la esfera cerca del suelo).
    terrains: Vec<Option<Terrain>>,
    use_terrain: Vec<bool>,
    /// Triángulos de terreno seleccionados en el último cuadro (para el HUD y las mediciones).
    pub terrain_triangles: u32,
}

impl Scene {
    pub fn new() -> Self {
        let bodies = solar_system();
        let ship = ShipModel::load("assets/models/ship.obj").expect("falta assets/models/ship.obj");
        Self {
            sky: SkyBox::new(1024, 0.0),
            shaders: bodies.iter().map(|b| shader_for(b.kind)).collect(),
            lods: bodies.iter().map(|b| crate::relief::displaced_lods(b.kind, 5)).collect(),
            levels: vec![0; bodies.len()],
            ring: ring_mesh(1.3, 2.1, 96),
            ring_shader: shader_for_ring(),
            ship,
            ship_shader: ship_shader(),
            terrains: bodies.iter().map(|b| b.landable.then(|| Terrain::new(b.kind, b.radius, TERRAIN_MAX_LEVEL))).collect(),
            use_terrain: vec![false; bodies.len()],
            terrain_triangles: 0,
        }
    }

    pub fn render(&mut self, fb: &mut Framebuffer, sim: &Sim, cam: &CameraRig, opts: &RenderOptions, lod_on: bool, stats: &mut Stats) {
        let eye = cam.eye(&sim.ship);
        let view = cam.view_matrix();
        let proj = Mat4::perspective(FOV_Y, fb.width as f32 / fb.height as f32, NEAR, FAR);
        let time = sim.t as f32;
        let t_sky = Instant::now();
        self.sky.paint(fb, view);
        stats.ms_sky += t_sky.elapsed().as_secs_f32() * 1000.0;

        let max_level = self.lods[0].levels.len() - 1;
        let mut dists = Vec::with_capacity(sim.bodies.len());
        for (i, b) in sim.bodies.iter().enumerate() {
            let dist = (b.position(sim.t) - eye).length() as f32;
            dists.push(dist);
            self.levels[i] = if lod_on {
                // Histéresis para no parpadear + cota por el relieve (la silueta deformada exige más detalle).
                let (r, h) = (b.radius as f32, fb.height as f32);
                let base = self.lods[i].select_with_hysteresis(self.levels[i], dist, r, FOV_Y, h, LOD_MARGIN);
                let relief = self.lods[i].select_with_relief(dist, r, FOV_Y, h, crate::relief::max_amplitude(b.kind));
                base.max(relief)
            } else {
                max_level
            };
        }

        // Terreno: cerca del suelo se dibujan chunks (en el marco local del planeta) en vez de la esfera.
        let q = cam.orientation();
        let (fwd_w, up_w) = (q.rotate(Vec3::new(0.0, 0.0, -1.0)).to_f64(), q.rotate(Vec3::Y).to_f64());
        let aspect = fb.width as f64 / fb.height as f64;
        let mut chunk_sets: Vec<(usize, Vec<(ChunkKey, Arc<Mesh>)>)> = Vec::new();
        self.terrain_triangles = 0;
        for (i, b) in sim.bodies.iter().enumerate() {
            let Some(terrain) = self.terrains[i].as_mut() else { continue };
            let alt = dists[i] as f64 / b.radius - 1.0;
            self.use_terrain[i] = if self.use_terrain[i] { alt < TERRAIN_OFF } else { alt < TERRAIN_ON };
            if !self.use_terrain[i] {
                continue;
            }
            let cam_local = b.world_to_fixed(sim.t, eye - b.position(sim.t));
            let frustum = Frustum::from_camera(cam_local, b.world_to_fixed(sim.t, fwd_w), b.world_to_fixed(sim.t, up_w), FOV_Y as f64, aspect);
            let chunks = terrain.frame(cam_local, FOV_Y as f64, fb.height as f64, Some(&frustum));
            self.terrain_triangles += terrain.stats.triangles;
            chunk_sets.push((i, chunks));
        }

        let mut calls: Vec<DrawCall> = Vec::new();
        for (i, b) in sim.bodies.iter().enumerate() {
            let pos = b.position(sim.t);
            let model = body_model_matrix(b, sim.t, eye);
            let light = if i == 0 { Vec3::Y } else { (-pos).normalized().to_f32() };
            let coverage = b.radius as f32 / dists[i].max(1.0);
            let detail = if lod_on { (coverage * 8.0).clamp(0.15, 1.0) } else { 1.0 };
            if !self.use_terrain[i] {
                calls.push(DrawCall {
                    mesh: &self.lods[i].levels[self.levels[i]],
                    model,
                    shader: self.shaders[i].as_ref(),
                    uniforms: Uniforms { light_dir: light, time, detail, model },
                });
            }
            if b.name == "Siroco" {
                let rel = (pos - eye).to_f32();
                let m = Mat4::translation(rel) * Mat4::rotation_z(0.35) * Mat4::uniform_scale(b.radius as f32);
                calls.push(DrawCall {
                    mesh: &self.ring,
                    model: m,
                    shader: self.ring_shader.as_ref(),
                    uniforms: Uniforms { light_dir: light, time, detail, model: m },
                });
            }
        }
        // Chunks de terreno: mallas en metros locales, modelo = traslación relativa al ojo * rotación del planeta (sin escala).
        for (i, chunks) in &chunk_sets {
            let b = &sim.bodies[*i];
            let model = Mat4::translation((b.position(sim.t) - eye).to_f32()) * Mat4::rotation_y(b.rotation_angle(sim.t) as f32);
            let light = (-b.position(sim.t)).normalized().to_f32();
            for (_, mesh) in chunks {
                calls.push(DrawCall {
                    mesh: mesh.as_ref(),
                    model,
                    shader: self.shaders[*i].as_ref(),
                    uniforms: Uniforms { light_dir: light, time, detail: 1.0, model },
                });
            }
        }
        // La nave del jugador (luz del sol hacia el origen).
        let rel = (sim.ship.position - eye).to_f32();
        let ship_model = Mat4::translation(rel) * sim.ship.orientation.to_mat4();
        let sun_dir = (-sim.ship.position).normalized().to_f32();
        let ship_uniforms = Uniforms { light_dir: sun_dir, time, detail: 1.0, model: ship_model };
        calls.extend(self.ship.draw_calls(ship_model, sim.ship.gear, self.ship_shader.as_ref(), ship_uniforms));
        render_scene(fb, view, proj, &calls, opts, stats);
        crate::atmosphere::apply(fb, view, FOV_Y, eye, &sim.bodies, sim.t);
    }
}

/// Velocidad respecto del cuerpo; en descenso se mide contra el SUELO (descuenta su rotación).
fn speed_relative(sim: &Sim, idx: usize) -> f64 {
    let b = &sim.bodies[idx];
    let mut v = sim.ship.velocity - b.velocity(sim.t);
    if sim.ship.mode == crate::ship::NavMode::Descent {
        v = v - b.spin_velocity(sim.ship.position - b.position(sim.t));
    }
    v.length()
}

fn hud_state<'a>(sim: &'a Sim, stats: &'a Stats, fps: f32, show_stats: bool, show_help: bool, opts: &RenderOptions, lod_on: bool) -> HudState<'a> {
    let near = nearest_body(&sim.bodies, sim.ship.position, sim.t);
    let nb = &sim.bodies[near];
    HudState {
        mode: if sim.autopilot.is_trip() { "VIAJE" } else if sim.autopilot.active { "AUTOPILOTO" } else { sim.ship.mode.name() },
        nearest_body: nb.name,
        altitude_m: altitude(nb.position(sim.t), nb.radius, sim.ship.position),
        speed_ms: if sim.ship.mode == crate::ship::NavMode::Landed { 0.0 } else { speed_relative(sim, near) },
        tour_active: sim.autopilot.active && sim.autopilot.tour,
        tour_status: if sim.autopilot.active { sim.autopilot.status() } else { "" },
        show_stats,
        show_help,
        stats,
        fps,
        culling_on: opts.frustum_culling,
        lod_on,
        land_active: sim.autopilot.is_landing(),
    }
}

/// Renderiza el Tour sin ventana y guarda un PNG cada `every` cuadros en `dir` (para revisar y armar el video).
pub fn record(dir: &str, frames: u32, every: u32) {
    std::fs::create_dir_all(dir).expect("no se pudo crear el directorio");
    let mut scene = Scene::new();
    let mut sim = new_sim();
    let mut cam = CameraRig::new();
    cam.snap(&sim.ship);
    sim.start_tour();
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    let opts = RenderOptions::default();
    for f in 0..frames {
        sim.frame(1.0 / 60.0, &Input::default());
        cam.update(1.0 / 60.0, &sim.ship, 0.0, 0.0, false);
        if f % every != 0 {
            continue;
        }
        let mut stats = Stats::default();
        scene.render(&mut fb, &sim, &cam, &opts, true, &mut stats);
        draw_hud_layout(&mut fb, &hud_state(&sim, &stats, 60.0, true, false, &opts, true), (-1.0, -1.0));
        let path = format!("{dir}/frame-{:06}.png", f / every);
        fb.save_png(Path::new(&path)).expect("no se pudo guardar el cuadro");
    }
    println!("{} cuadros guardados en {dir} (t = {:.1} s de simulación, visitados: {})", frames.div_ceil(every), sim.t, sim.autopilot.visited.len());
}

pub fn run(start_tour: bool) {
    let mut platform = Platform::new(WIDTH, HEIGHT, "Galaxy | renderizador por software");
    let mut scene = Scene::new();
    let mut sim = new_sim();
    let mut cam = CameraRig::new();
    cam.snap(&sim.ship);
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    let (mut show_stats, mut show_help) = (false, false);
    let mut opts = RenderOptions::default();
    let mut lod_on = true;
    let mut fps = 60.0f32;
    let mut layout: Option<HudLayout> = None;
    let mut shots = 0u32;
    if start_tour {
        sim.start_tour();
    }
    while !platform.should_close() {
        let input = platform.poll();
        if input.quit {
            break;
        }
        show_help ^= input.toggle_help;
        show_stats ^= input.toggle_stats;
        if input.toggle_culling {
            opts.frustum_culling = !opts.frustum_culling;
        }
        if input.toggle_lod {
            lod_on = !lod_on;
        }
        if input.toggle_normalmap {
            crate::shading::set_normal_map(!crate::shading::normal_map_enabled());
        }
        if input.toggle_gravity {
            sim.ship.gravity_enabled = !sim.ship.gravity_enabled;
        }
        if input.toggle_assist {
            sim.ship.flight_assist = !sim.ship.flight_assist;
        }
        if input.toggle_tour {
            sim.toggle_tour();
        }
        if input.land_auto {
            sim.land_here();
        }
        if let Some(i) = input.select_body {
            sim.go_to(i);
        }
        if input.mouse_clicked {
            if let Some(l) = &layout {
                if l.tour.contains(input.mouse.0, input.mouse.1) {
                    sim.toggle_tour();
                } else if l.land.contains(input.mouse.0, input.mouse.1) {
                    sim.land_here();
                }
            }
        }
        let dt = platform.frame_time().clamp(0.0, 0.1);
        sim.frame(dt, &input.ship);
        cam.update(dt, &sim.ship, input.mouse_look.0, input.mouse_look.1, input.free_look);
        if dt > 1e-4 {
            fps += ((1.0 / dt as f32) - fps) * 0.1;
        }

        let mut stats = Stats::default();
        scene.render(&mut fb, &sim, &cam, &opts, lod_on, &mut stats);

        let state = hud_state(&sim, &stats, fps, show_stats, show_help, &opts, lod_on);
        layout = Some(draw_hud_layout(&mut fb, &state, input.mouse));
        if sim.autopilot.active {
            crate::hud::draw_nav_panel(&mut fb, sim.autopilot.target_name(&sim.bodies), sim.autopilot.status(), sim.autopilot.eta());
        }

        if input.screenshot {
            let _ = std::fs::create_dir_all("screenshots");
            shots += 1;
            let path = format!("screenshots/vista-{shots}.png");
            if fb.save_png(Path::new(&path)).is_ok() {
                println!("captura guardada en {path}");
            }
        }
        platform.present(&fb);
    }
}

/// Banco de pruebas sin ventana: el mismo recorrido (Tour) con y sin culling/LOD.
pub fn bench(frames: u32) {
    println!("Recorrido del Tour: {frames} cuadros a 60 Hz de simulación, {WIDTH}x{HEIGHT}");
    println!("{:<26} {:>9} {:>9} {:>12} {:>12} {:>10}", "configuración", "ms medio", "ms máx", "tris entrada", "tris dibuj.", "calls cull");
    for (name, on) in [("culling + LOD", true), ("sin culling, sin LOD", false)] {
        let mut scene = Scene::new();
        let mut sim = new_sim();
        let mut cam = CameraRig::new();
        cam.snap(&sim.ship);
        sim.start_tour();
        let mut fb = Framebuffer::new(WIDTH, HEIGHT);
        let opts = RenderOptions { frustum_culling: on, backface_culling: true };
        let (mut total_ms, mut max_ms) = (0.0f64, 0.0f64);
        let (mut tin, mut tdrawn, mut culled) = (0u64, 0u64, 0u64);
        let (mut m_sky, mut m_v, mut m_r, mut m_s) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for _ in 0..frames {
            sim.frame(1.0 / 60.0, &Input::default());
            cam.update(1.0 / 60.0, &sim.ship, 0.0, 0.0, false);
            let mut stats = Stats::default();
            let t0 = Instant::now();
            scene.render(&mut fb, &sim, &cam, &opts, on, &mut stats);
            let ms = t0.elapsed().as_secs_f64() * 1000.0;
            total_ms += ms;
            max_ms = max_ms.max(ms);
            tin += stats.triangles_in;
            tdrawn += stats.triangles_drawn;
            culled += stats.draw_calls_culled as u64;
            m_sky += stats.ms_sky;
            m_v += stats.ms_vertex;
            m_r += stats.ms_raster;
            m_s += stats.ms_shade;
        }
        let n = frames as u64;
        println!(
            "{:<26} {:>9.2} {:>9.2} {:>12} {:>12} {:>10}",
            name,
            total_ms / frames as f64,
            max_ms,
            tin / n,
            tdrawn / n,
            culled / n
        );
        let f = frames as f32;
        println!("    etapas (ms/cuadro): cielo {:.2}  vértices {:.2}  raster {:.2}  sombreado {:.2}", m_sky / f, m_v / f, m_r / f, m_s / f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubemap_round_trips_texel_centers() {
        let sky = SkyBox::new(16, 0.0);
        for face in 0..6 {
            for (x, y) in [(0, 0), (5, 9), (15, 15), (8, 3)] {
                let sc = (x as f32 + 0.5) / 16.0 * 2.0 - 1.0;
                let tc = (y as f32 + 0.5) / 16.0 * 2.0 - 1.0;
                let d = face_dir(face, sc, tc);
                let (f, u, v) = face_uv(d);
                assert_eq!(f, face);
                assert_eq!(((u * 16.0) as usize, (v * 16.0) as usize), (x, y));
                assert_eq!(sky.sample(d), sky.faces[face][y * 16 + x]);
            }
        }
    }
}
