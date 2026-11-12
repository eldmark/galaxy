//! Modos de verificación sin ventana: `--orbits-shot`, `--death-shot`, `--landing-shot`.
//! Cada uno monta un escenario determinista con la simulación real y guarda PNG con todo el pipeline (escena, efectos, HUD).
use crate::app::{new_sim, Scene, HEIGHT, WIDTH};
use crate::fx::FlightFx;
use crate::hud::{draw_hud_layout, HudState};
use crate::math::{DVec3, Quat, Vec3};
use crate::raster::{Framebuffer, RenderOptions, Stats};
use crate::ship::{CameraRig, Input, NavMode, Sim};
use std::path::Path;

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

/// Atiende los modos de demostración. Devuelve true si ejecutó alguno.
pub fn dispatch(args: &[String]) -> bool {
    if let Some(out) = arg_value(args, "--orbits-shot") {
        orbits_shot(Path::new(&out));
        return true;
    }
    if let Some(dir) = arg_value(args, "--death-shot") {
        death_shot(&dir);
        return true;
    }
    if let Some(dir) = arg_value(args, "--landing-shot") {
        landing_shot(&dir);
        return true;
    }
    false
}

struct Rig {
    scene: Scene,
    fb: Framebuffer,
    cam: CameraRig,
    fx: FlightFx,
    opts: RenderOptions,
}

impl Rig {
    fn new(sim: &Sim) -> Self {
        let mut cam = CameraRig::new();
        cam.snap(&sim.ship);
        Self { scene: Scene::new(), fb: Framebuffer::new(WIDTH, HEIGHT), cam, fx: FlightFx::new(), opts: RenderOptions::default() }
    }

    /// Un cuadro de simulación a 60 Hz con la entrada dada.
    fn step(&mut self, sim: &mut Sim, input: &Input) {
        sim.frame(1.0 / 60.0, input);
        self.cam.update(1.0 / 60.0, &sim.ship, 0.0, 0.0, false);
        self.fx.update(sim, &mut self.cam, 1.0 / 60.0, false, false);
    }

    fn shot(&mut self, sim: &Sim, path: &str) {
        let mut stats = Stats::default();
        self.scene.render(&mut self.fb, sim, &self.cam, &self.opts, true, &mut stats);
        self.fx.draw_world(&mut self.fb, sim, &self.cam);
        let near = crate::world::nearest_body(&sim.bodies, sim.ship.position, sim.t);
        let nb = &sim.bodies[near];
        let state = HudState {
            mode: sim.ship.mode.name(),
            nearest_body: nb.name,
            altitude_m: crate::world::altitude(nb.position(sim.t), nb.radius, sim.ship.position),
            speed_ms: (sim.ship.velocity - nb.velocity(sim.t)).length(),
            tour_active: false,
            spectator: false,
            tour_available: true,
            tour_status: "",
            show_stats: false,
            show_help: false,
            stats: &stats,
            terrain: &self.scene.terrain_stats,
            scatter: &self.scene.scatter_stats,
            scale: 1.0,
            scale_auto: false,
            fps: 60.0,
            culling_on: true,
            lod_on: true,
            land_active: sim.autopilot.is_landing(),
        };
        draw_hud_layout(&mut self.fb, &state, (-1.0, -1.0));
        self.fx.draw_hud(&mut self.fb, sim);
        self.fb.save_png(Path::new(path)).expect("no se pudo guardar el cuadro");
    }
}

/// Vista cenital de todo el sistema con las líneas de órbita.
pub fn orbits_shot(out: &Path) {
    let mut sim = new_sim();
    sim.ship.position = DVec3::new(0.0, 250_000.0, 0.0);
    sim.ship.velocity = DVec3::ZERO;
    sim.ship.orientation = Quat::from_axis_angle(Vec3::X, -std::f32::consts::FRAC_PI_2);
    let mut rig = Rig::new(&sim);
    rig.cam.offset = Vec3::new(0.0, 0.0, -3000.0); // cámara por delante de la nave: la nave queda a su espalda
    rig.cam.snap(&sim.ship);
    sim.ship.gravity_enabled = false;
    sim.frame(1.0 / 60.0, &Input::default());
    rig.shot(&sim, &out.to_string_lossy());
    println!("guardado {}", out.display());
}

/// Acerca la nave al sol con boost hasta morir; guarda ~un cuadro cada 0.25 s desde que sube el calor hasta la reaparición.
pub fn death_shot(dir: &str) {
    std::fs::create_dir_all(dir).expect("no se pudo crear el directorio");
    let mut sim = new_sim();
    let r = sim.bodies[0].radius;
    sim.ship.position = DVec3::new(0.0, 0.0, 3.6 * r);
    sim.ship.velocity = DVec3::ZERO;
    let mut rig = Rig::new(&sim);
    let fwd = Input { thrust: Vec3::new(0.0, 0.0, 1.0), boost: true, ..Default::default() };
    let mut n = 0;
    let mut dead_frames = 0;
    for f in 0..(60 * 30) {
        let inp = if sim.is_alive() { fwd } else { Input::default() };
        rig.step(&mut sim, &inp);
        let interesting = sim.heat > 0.02 || !sim.is_alive();
        if !sim.is_alive() {
            dead_frames += 1;
        }
        let snap = if dead_frames > 0 { [3, 8, 20, 40, 70, 110, 150, 200, 235].contains(&dead_frames) } else { f % 15 == 0 };
        if interesting && snap {
            rig.shot(&sim, &format!("{dir}/death-{n:03}.png"));
            n += 1;
        }
        if sim.is_alive() && dead_frames > 0 && (sim.t - sim.respawned_at) > 1.2 {
            break;
        }
    }
    rig.shot(&sim, &format!("{dir}/death-{n:03}.png"));
    println!("{} cuadros en {dir}", n + 1);
}

/// Descenso lento a Verdalia: el aterrizaje asistido se activa solo; guarda cuadros hasta posarse.
pub fn landing_shot(dir: &str) {
    std::fs::create_dir_all(dir).expect("no se pudo crear el directorio");
    let mut sim = new_sim();
    let b = &sim.bodies[2];
    let n = DVec3::new(0.3, 0.9, 0.3).normalized();
    let rel = n * (crate::world::surface_height_at(b, 0.0, n) + 235.0);
    sim.ship.position = b.position(0.0) + rel;
    sim.ship.velocity = b.velocity(0.0) + b.spin_velocity(rel) - n * 12.0;
    sim.ship.mode = NavMode::Descent;
    sim.ship.frame = Some(2);
    sim.ship.takeoff_cooldown = 0.0;
    // orientación con el "arriba" de la nave = normal local y el morro hacia el horizonte
    sim.ship.orientation = Quat::from_axis_angle(Vec3::Y, 0.0);
    let up = n.to_f32();
    let axis = Vec3::Y.cross(up);
    if axis.length() > 1e-6 {
        sim.ship.orientation = Quat::from_axis_angle(axis, Vec3::Y.dot(up).clamp(-1.0, 1.0).acos());
    }
    let mut rig = Rig::new(&sim);
    rig.cam.offset = Vec3::new(0.0, 6.0, 30.0);
    rig.cam.snap(&sim.ship);
    let mut k = 0;
    let mut landed_at = None;
    for f in 0..(60 * 90) {
        rig.cam.offset = Vec3::new(0.0, 6.0, 30.0);
        rig.step(&mut sim, &Input::default());
        if sim.ship.mode == NavMode::Landed && landed_at.is_none() {
            landed_at = Some(f);
        }
        if f % 30 == 0 && f > 0 {
            rig.shot(&sim, &format!("{dir}/landing-{k:03}.png"));
            k += 1;
        }
        if landed_at.is_some_and(|l| f > l + 120) {
            break;
        }
    }
    println!("{k} cuadros en {dir}; posada en el cuadro {landed_at:?}");
}
