//! `Sim`: pegamento de conveniencia para el coordinador (y las pruebas headless).
//! Une mundo, nave, autopiloto, paso fijo y peligros (calor del sol, impacto, aterrizaje asistido).
//! El coordinador solo llama `frame(dt_real, entrada)`.
use super::hazard::{heat_damage_rate, heat_target, segment_sphere_entry, DeathCause, ShipStatus, SimEvent};
use super::modes::*;
use super::{Autopilot, FixedStepper, Input, NavMode, Ship};
use crate::math::DVec3;
use crate::world::Body;

pub struct Sim {
    pub bodies: Vec<Body>,
    pub ship: Ship,
    pub autopilot: Autopilot,
    pub stepper: FixedStepper,
    /// Tiempo de simulación (s): siempre múltiplo del paso fijo.
    pub t: f64,
    /// Vivo o destruido (con causa y momento).
    pub status: ShipStatus,
    /// Temperatura 0..1 por cercanía al sol (medidor del HUD).
    pub heat: f64,
    /// Integridad del casco 1..0 (calor y rebotes la reducen; 0 = destruida).
    pub hull: f64,
    /// Tiempo de simulación del último reaparecer (para el fundido de entrada).
    pub respawned_at: f64,
    /// Eventos pendientes (vaciar con `take_events`).
    pub events: Vec<SimEvent>,
    /// Punto de reaparición: (cuerpo, desplazamiento respecto de él). Hereda la velocidad del cuerpo.
    spawn: Option<(usize, DVec3)>,
    spawn_pos: DVec3,
    /// Segundos restantes en que el aterrizaje asistido no se reactiva (tras cancelarlo).
    assist_block: f64,
    /// Ancla del naufragio (cuerpo, posición fija al cuerpo) para que siga al planeta tras un impacto.
    wreck: Option<(usize, DVec3)>,
}

impl Sim {
    pub fn new(bodies: Vec<Body>, ship_pos: DVec3) -> Self {
        Self {
            bodies,
            ship: Ship::new(ship_pos),
            autopilot: Autopilot::new(),
            stepper: FixedStepper::default_120(),
            t: 0.0,
            status: ShipStatus::Alive,
            heat: 0.0,
            hull: 1.0,
            respawned_at: -100.0,
            events: Vec::new(),
            spawn: None,
            spawn_pos: ship_pos,
            assist_block: 0.0,
            wreck: None,
        }
    }

    /// Punto de reaparición relativo a un cuerpo (como `app::new_sim`).
    pub fn set_spawn(&mut self, body: usize, offset: DVec3) {
        self.spawn = Some((body, offset));
    }

    pub fn is_alive(&self) -> bool {
        self.status == ShipStatus::Alive
    }

    pub fn take_events(&mut self) -> Vec<SimEvent> {
        std::mem::take(&mut self.events)
    }

    /// Destruye la nave (idempotente).
    pub fn kill(&mut self, cause: DeathCause) {
        if !self.is_alive() {
            return;
        }
        self.status = ShipStatus::Dead { cause, since: self.t };
        self.autopilot.stop();
        self.ship.raw_controls = false;
        self.wreck = self.ship.contact.map(|i| {
            let b = &self.bodies[i];
            (i, b.world_to_fixed(self.t, self.ship.position - b.position(self.t)))
        });
        self.events.push(SimEvent::Destroyed { cause });
    }

    /// Reaparece en el punto inicial (velocidad del cuerpo de referencia o cero). Tecla K / Supr y automático.
    pub fn respawn(&mut self) {
        let (pos, vel) = match self.spawn {
            Some((i, off)) => (self.bodies[i].position(self.t) + off, self.bodies[i].velocity(self.t)),
            None => (self.spawn_pos, DVec3::ZERO),
        };
        self.ship = Ship::new(pos);
        self.ship.velocity = vel;
        self.autopilot.stop();
        self.status = ShipStatus::Alive;
        self.heat = 0.0;
        self.hull = 1.0;
        self.wreck = None;
        self.assist_block = 0.0;
        self.respawned_at = self.t;
        self.events.push(SimEvent::Respawned);
    }

    /// Avanza un cuadro de `frame_dt` s reales: ejecuta los pasos fijos necesarios.
    /// Si el jugador manda entrada mientras el autopiloto está activo, este se interrumpe
    /// y el control vuelve al jugador sin saltos (la nave conserva su estado).
    /// Devuelve el número de pasos fijos ejecutados.
    pub fn frame(&mut self, frame_dt: f64, player: &Input) -> u32 {
        let n = self.stepper.advance(frame_dt);
        let dt = self.stepper.dt;
        if self.autopilot.active && player.is_active() {
            if self.autopilot.assisted {
                self.assist_block = ASSIST_BLOCK_S;
                self.events.push(SimEvent::AssistCancelled);
            }
            self.autopilot.stop();
        }
        for _ in 0..n {
            if let ShipStatus::Dead { since, .. } = self.status {
                self.step_wreck();
                self.t += dt;
                if self.t - since >= RESPAWN_DELAY {
                    self.respawn();
                }
                continue;
            }
            self.assist_block = (self.assist_block - dt).max(0.0);
            self.try_assisted_landing(player);
            let input = if self.autopilot.active {
                self.ship.raw_controls = true;
                self.autopilot.update(dt, self.t, &self.ship, &self.bodies)
            } else {
                self.ship.raw_controls = false;
                *player
            };
            let p_old = self.ship.position;
            let was_landed = self.ship.mode == NavMode::Landed;
            self.ship.step(dt, self.t, &input, &self.bodies);
            self.t += dt;
            self.hazards(dt, p_old, was_landed);
        }
        if !self.autopilot.active {
            self.ship.raw_controls = false;
        }
        n
    }

    /// El naufragio se queda quieto (sol) o sigue al planeta contra el que chocó.
    fn step_wreck(&mut self) {
        self.ship.velocity = DVec3::ZERO;
        if let Some((i, local)) = self.wreck {
            let b = &self.bodies[i];
            self.ship.position = b.position(self.t) + b.fixed_to_world(self.t, local);
            self.ship.velocity = b.velocity(self.t);
        }
    }

    /// Calor del sol, impactos y estado de casco tras un paso.
    fn hazards(&mut self, dt: f64, p_old: DVec3, was_landed: bool) {
        let sun = &self.bodies[0];
        let sun_pos = sun.position(self.t);
        let d = (self.ship.position - sun_pos).length();
        // zona letal con barrido (a miles de m/s un paso mide cientos de metros)
        let lethal = SUN_KILL_R * sun.radius;
        if let Some(tau) = segment_sphere_entry(p_old, self.ship.position, sun_pos, lethal) {
            self.ship.position = p_old + (self.ship.position - p_old) * tau;
            self.heat = 1.0;
            self.kill(DeathCause::Heat);
            return;
        }
        let target = heat_target(d, sun.radius);
        let tau = if target > self.heat { HEAT_RISE_TAU } else { HEAT_FALL_TAU };
        self.heat += (target - self.heat) * (1.0 - (-dt / tau).exp());
        self.hull -= heat_damage_rate(self.heat) * dt;
        if self.hull <= 0.0 {
            self.hull = 0.0;
            self.kill(DeathCause::Heat);
            return;
        }
        // impactos
        if let Some(i) = self.ship.contact {
            let v = self.ship.contact_speed;
            if v > IMPACT_DEATH_SPEED {
                self.kill(DeathCause::Impact);
                return;
            } else if v > BOUNCE_MIN_SPEED {
                self.hull -= (v - BOUNCE_MIN_SPEED) / (IMPACT_DEATH_SPEED - BOUNCE_MIN_SPEED) * 0.45;
                self.events.push(SimEvent::Bounce { body: i, speed: v });
                if self.hull <= 0.0 {
                    self.hull = 0.0;
                    self.kill(DeathCause::Impact);
                    return;
                }
            }
        }
        if self.ship.mode == NavMode::Landed {
            self.hull = (self.hull + 0.05 * dt).min(1.0);
            if !was_landed {
                self.events.push(SimEvent::Touchdown { body: self.ship.frame.unwrap_or(0) });
            }
        }
    }

    /// Aterrizaje asistido: en DESCENSO sobre un planeta aterrizable, por debajo de ASSIST_LAND_ALT, bajando a menos de
    /// ASSIST_LAND_VDOWN, con poca velocidad horizontal y sin mandos del jugador, arranca solo la secuencia de
    /// aterrizaje del autopiloto. Cualquier mando del jugador la cancela (ver `frame`) y la bloquea ASSIST_BLOCK_S.
    fn try_assisted_landing(&mut self, player: &Input) {
        let s = &self.ship;
        if self.autopilot.active || self.assist_block > 0.0 || player.is_active() || player.boost {
            return;
        }
        if s.mode != NavMode::Descent || s.takeoff_cooldown > 0.0 {
            return;
        }
        let Some(i) = s.frame else { return };
        let b = &self.bodies[i];
        if !b.landable || s.altitude >= ASSIST_LAND_ALT {
            return;
        }
        let rel = s.position - b.position(self.t);
        let n = rel.normalized();
        let v = s.velocity - b.velocity(self.t) - b.spin_velocity(rel);
        let v_down = -v.dot(n);
        let v_h = (v - n * v.dot(n)).length();
        if v_down < 0.0 || v_down > ASSIST_LAND_VDOWN || v_h > ASSIST_LAND_VHORIZ {
            return;
        }
        if self.autopilot.start_assisted_landing(&self.ship, &self.bodies, self.t) {
            self.events.push(SimEvent::AssistStarted);
        }
    }

    pub fn start_tour(&mut self) {
        self.autopilot.start(&self.ship, &self.bodies, self.t);
    }
    /// Alterna el Tour (tecla T / botón).
    pub fn toggle_tour(&mut self) {
        if self.autopilot.active {
            self.autopilot.stop();
            self.ship.raw_controls = false;
        } else {
            self.start_tour();
        }
    }
    /// Viaje automático al cuerpo `idx` (teclas 1..6 -> 0..=5; 0 = sol). Termina en una vuelta de órbita y
    /// devuelve el control. Devuelve false si el índice no existe.
    pub fn go_to(&mut self, idx: usize) -> bool {
        self.autopilot.go_to(idx, &self.ship, &self.bodies, self.t)
    }
    /// Aterrizaje automático en el planeta actual. Devuelve false si no hay un cuerpo aterrizable cerca.
    pub fn land_here(&mut self) -> bool {
        self.autopilot.start_landing_here(&self.ship, &self.bodies, self.t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Vec3;
    use crate::world::solar_system;

    /// Simula 600 pasos fijos (5 s) alimentados con distintos dt de cuadro.
    fn run_with_frame_dt(fdt: f64) -> Ship {
        let bodies = solar_system();
        let b = &bodies[2];
        let p = b.position(0.0);
        let mut sim = Sim::new(bodies, p + DVec3::new(0.0, 0.0, 3.0 * 3500.0));
        sim.ship.velocity = sim.bodies[2].velocity(0.0);
        let inp = Input {
            thrust: Vec3::new(0.3, 0.0, 0.8),
            rotate: Vec3::new(0.2, 0.5, 0.1),
            ..Default::default()
        };
        let mut done = 0u32;
        while done < 600 {
            let n = sim.stepper.advance(fdt);
            let dt = sim.stepper.dt;
            for _ in 0..n {
                if done < 600 {
                    sim.ship.step(dt, sim.t, &inp, &sim.bodies);
                    sim.t += dt;
                    done += 1;
                }
            }
        }
        sim.ship
    }

    #[test]
    fn fixed_step_is_frame_rate_independent() {
        let a = run_with_frame_dt(1.0 / 20.0);
        let b = run_with_frame_dt(1.0 / 60.0);
        let c = run_with_frame_dt(1.0 / 144.0);
        let d = run_with_frame_dt(0.013);
        for s in [&b, &c, &d] {
            assert!((a.position - s.position).length() < 1e-6);
            assert!((a.velocity - s.velocity).length() < 1e-6);
        }
        assert!(a.velocity.length() > 1.0);
    }
}
