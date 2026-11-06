//! Efectos de vuelo: une eventos de `Sim` (explosión, polvo, reaparición), líneas de órbita, tinte de calor,
//! destello/fundido y HUD de vuelo. El bucle solo llama `update`, `draw_world` y `draw_hud`.
use crate::hud::{draw_flight_hud, FlightHud};
use crate::math::{DVec3, Mat4};
use crate::orbits::{draw_orbits, orbit_ref};
use crate::particles::{dust_color, death_fade_alpha, fade, flash_alpha, heat_tint, respawn_fade_alpha, Particles};
use crate::raster::Framebuffer;
use crate::ship::modes::*;
use crate::ship::{CameraRig, NavMode, ShipStatus, Sim, SimEvent};
use crate::world::surface_height_at;

pub struct FlightFx {
    pub particles: Particles,
    pub show_orbits: bool,
    dust_acc: f32,
    seed: u32,
}

impl Default for FlightFx {
    fn default() -> Self {
        Self::new()
    }
}

/// Offset de cámara normal y de duelo (se aleja para ver la explosión).
const CAM_NORMAL: (f32, f32, f32) = (0.0, 7.0, 26.0);
const CAM_DEAD: (f32, f32, f32) = (0.0, 16.0, 110.0);

impl FlightFx {
    pub fn new() -> Self {
        Self { particles: Particles::new(), show_orbits: true, dust_acc: 0.0, seed: 1 }
    }

    /// Una vez por cuadro tras `Sim::frame`: procesa eventos, emite polvo y avanza partículas.
    pub fn update(&mut self, sim: &mut Sim, cam: &mut CameraRig, dt: f64, toggle_orbits: bool, respawn: bool) {
        if toggle_orbits {
            self.show_orbits = !self.show_orbits;
        }
        if respawn {
            sim.respawn();
        }
        for e in sim.take_events() {
            self.seed = self.seed.wrapping_add(1);
            let ship = &sim.ship;
            match e {
                SimEvent::Destroyed { .. } => {
                    // choque: el fuego va en el marco del planeta (lo sigue); calor: en el del mundo
                    match ship.contact {
                        Some(i) => {
                            let b = &sim.bodies[i];
                            let local = b.world_to_fixed(sim.t, ship.position - b.position(sim.t));
                            self.particles.emit_explosion(Some(i), local, DVec3::ZERO, 260, self.seed);
                        }
                        None => self.particles.emit_explosion(None, ship.position, ship.velocity, 260, self.seed),
                    }
                }
                SimEvent::Touchdown { body } => self.dust_puff(sim, body, 60, 0.6),
                SimEvent::Bounce { body, speed } => self.dust_puff(sim, body, 50, (speed / 60.0).clamp(0.3, 1.0) as f32),
                SimEvent::Respawned => self.particles.clear(),
                SimEvent::AssistStarted | SimEvent::AssistCancelled => {}
            }
        }
        // polvo continuo al volar bajo con empuje (despegue, aterrizaje, vuelo rasante)
        let ship = &sim.ship;
        if sim.is_alive() && matches!(ship.mode, NavMode::Descent | NavMode::Landed) {
            if let Some(i) = ship.frame {
                if sim.bodies[i].landable && ship.altitude < 45.0 && ship.mode != NavMode::Landed {
                    let near = 1.0 - (ship.altitude / 45.0).clamp(0.0, 1.0);
                    let push = (ship.accel.length() / 25.0).clamp(0.0, 1.0);
                    self.dust_acc += (110.0 * near * push) as f32 * dt as f32;
                    let n = self.dust_acc as usize;
                    self.dust_acc -= n as f32;
                    if n > 0 {
                        self.dust_puff(sim, i, n, 0.5 + 0.5 * near as f32);
                    }
                }
            }
        }
        self.particles.update(dt as f32);

        // la cámara se aleja al morir y vuelve al reaparecer
        let target = if sim.is_alive() { CAM_NORMAL } else { CAM_DEAD };
        let k = 1.0 - (-3.0 * dt as f32).exp();
        let o = cam.offset;
        cam.offset = crate::math::Vec3::new(o.x + (target.0 - o.x) * k, o.y + (target.1 - o.y) * k, o.z + (target.2 - o.z) * k);
        if matches!(e_respawned(sim), true) {
            cam.snap(&sim.ship);
        }
    }

    /// Polvo en el suelo bajo la nave (ceniza/arena/nieve según el cuerpo).
    fn dust_puff(&mut self, sim: &Sim, body: usize, n: usize, strength: f32) {
        let b = &sim.bodies[body];
        let rel = sim.ship.position - b.position(sim.t);
        let dir = rel.normalized();
        let ground = dir * surface_height_at(b, sim.t, dir);
        let local = b.world_to_fixed(sim.t, ground);
        let up = b.world_to_fixed(sim.t, dir);
        self.seed = self.seed.wrapping_add(7);
        self.particles.emit_dust(body, local, up, dust_color(b.kind), n, strength, self.seed);
    }

    /// Tras `Scene::render` y antes del HUD: órbitas, partículas y tinte de calor.
    pub fn draw_world(&self, fb: &mut Framebuffer, sim: &Sim, cam: &CameraRig) {
        let eye = cam.eye(&sim.ship);
        let view = cam.view_matrix();
        let proj = Mat4::perspective(crate::app::FOV_Y, fb.width as f32 / fb.height as f32, 1.0, 600_000.0);
        if self.show_orbits {
            draw_orbits(fb, view, proj, eye, &sim.bodies, sim.t);
        }
        self.particles.draw(fb, &view, &proj, eye, &sim.bodies, sim.t);
        if sim.is_alive() {
            heat_tint(fb, sim.heat as f32);
        }
    }

    /// Datos del HUD de vuelo.
    pub fn hud(&self, sim: &Sim) -> FlightHud {
        let s = &sim.ship;
        let orbit = if sim.is_alive() && matches!(s.mode, NavMode::Cruise | NavMode::Orbit) {
            orbit_ref(&sim.bodies, s.position, s.velocity, sim.t)
                .map(|o| (sim.bodies[o.body].name.to_uppercase(), o.v_rel, o.v_circ, o.stable))
        } else {
            None
        };
        FlightHud {
            heat: sim.heat as f32,
            hull: sim.hull as f32,
            orbit,
            nav_panel: sim.autopilot.active,
            landing_assist: sim.autopilot.is_landing() && sim.is_alive(),
            altitude: s.altitude,
            show_alt_bar: sim.is_alive() && s.mode == NavMode::Descent && s.altitude < ASSIST_LAND_ALT,
            dead: match sim.status {
                ShipStatus::Dead { cause, since } => Some((cause.message(), RESPAWN_DELAY - (sim.t - since))),
                ShipStatus::Alive => None,
            },
            time: sim.t,
        }
    }

    /// Tras el HUD base: HUD de vuelo y las pasadas de destello/fundido.
    pub fn draw_hud(&self, fb: &mut Framebuffer, sim: &Sim) {
        let hud = self.hud(sim);
        match sim.status {
            ShipStatus::Dead { since, .. } => {
                let age = sim.t - since;
                fade(fb, [255, 250, 235], flash_alpha(age));
                fade(fb, [0, 0, 0], death_fade_alpha(age));
                draw_flight_hud(fb, &hud);
            }
            ShipStatus::Alive => {
                draw_flight_hud(fb, &hud);
                fade(fb, [0, 0, 0], respawn_fade_alpha(sim.t - sim.respawned_at));
            }
        }
    }
}

/// ¿Acaba de reaparecer la nave en este cuadro? (para encajar la cámara sin retardo)
fn e_respawned(sim: &Sim) -> bool {
    sim.is_alive() && (sim.t - sim.respawned_at).abs() < 0.05
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;
    use crate::ship::Input;

    #[test]
    fn death_emits_explosion_and_respawn_clears_it() {
        let mut sim = crate::app::new_sim();
        let r = sim.bodies[0].radius;
        sim.ship.position = DVec3::new(0.0, 0.0, 3.0 * r);
        sim.ship.velocity = DVec3::new(0.0, 0.0, -12000.0);
        let mut cam = CameraRig::new();
        let mut fx = FlightFx::new();
        let mut max_particles = 0;
        for _ in 0..(60 * 2) {
            sim.frame(1.0 / 60.0, &Input { thrust: Vec3::new(0.0, 0.0, 1.0), ..Default::default() });
            fx.update(&mut sim, &mut cam, 1.0 / 60.0, false, false);
            max_particles = max_particles.max(fx.particles.len());
        }
        assert!(!sim.is_alive());
        assert!((150..=300).contains(&max_particles), "{max_particles}");
        assert!(fx.hud(&sim).dead.is_some());
        fx.update(&mut sim, &mut cam, 1.0 / 60.0, false, true); // K: reaparecer manualmente
        assert!(sim.is_alive());
        assert_eq!(fx.particles.len(), 0);
        assert!(fx.hud(&sim).dead.is_none());
        fx.update(&mut sim, &mut cam, 1.0 / 60.0, true, false);
        assert!(!fx.show_orbits);
    }

    #[test]
    fn touchdown_raises_dust_with_planet_color() {
        let bodies = crate::world::solar_system();
        let b = &bodies[2];
        let n = DVec3::new(0.0, 1.0, 0.0);
        let h = surface_height_at(b, 0.0, n);
        let mut sim = Sim::new(bodies.clone(), sim_pos(b, n, h + 20.0));
        sim.ship.mode = NavMode::Descent;
        sim.ship.frame = Some(2);
        sim.ship.velocity = sim.bodies[2].velocity(0.0) + sim.bodies[2].spin_velocity(n * (h + 20.0)) - n * 4.0;
        let mut cam = CameraRig::new();
        let mut fx = FlightFx::new();
        let mut touched = false;
        for _ in 0..(60 * 30) {
            sim.frame(1.0 / 60.0, &Input::default());
            touched |= sim.ship.mode == NavMode::Landed;
            fx.update(&mut sim, &mut cam, 1.0 / 60.0, false, false);
            if touched && fx.particles.len() > 0 {
                break;
            }
        }
        assert!(touched && fx.particles.len() > 0, "sin polvo al tocar");
        assert!(fx.particles.list.iter().all(|p| p.body == Some(2)));
    }

    fn sim_pos(b: &crate::world::Body, n: DVec3, r: f64) -> DVec3 {
        b.position(0.0) + n * r
    }
}
