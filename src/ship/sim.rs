//! `Sim`: pegamento de conveniencia para el coordinador (y las pruebas headless).
//! Une mundo, nave, autopiloto y paso fijo. El coordinador solo llama `frame(dt_real, entrada)`.
use super::{Autopilot, FixedStepper, Input, Ship};
use crate::math::DVec3;
use crate::world::Body;

pub struct Sim {
    pub bodies: Vec<Body>,
    pub ship: Ship,
    pub autopilot: Autopilot,
    pub stepper: FixedStepper,
    /// Tiempo de simulación (s): siempre múltiplo del paso fijo.
    pub t: f64,
}

impl Sim {
    pub fn new(bodies: Vec<Body>, ship_pos: DVec3) -> Self {
        Self {
            bodies,
            ship: Ship::new(ship_pos),
            autopilot: Autopilot::new(),
            stepper: FixedStepper::default_120(),
            t: 0.0,
        }
    }

    /// Avanza un cuadro de `frame_dt` s reales: ejecuta los pasos fijos necesarios.
    /// Si el jugador manda entrada mientras el autopiloto está activo, este se interrumpe
    /// y el control vuelve al jugador sin saltos (la nave conserva su estado).
    /// Devuelve el número de pasos fijos ejecutados.
    pub fn frame(&mut self, frame_dt: f64, player: &Input) -> u32 {
        let n = self.stepper.advance(frame_dt);
        let dt = self.stepper.dt;
        if self.autopilot.active && player.is_active() {
            self.autopilot.stop();
        }
        for _ in 0..n {
            let input = if self.autopilot.active {
                self.ship.raw_controls = true;
                self.autopilot.update(dt, self.t, &self.ship, &self.bodies)
            } else {
                self.ship.raw_controls = false;
                *player
            };
            self.ship.step(dt, self.t, &input, &self.bodies);
            self.t += dt;
        }
        if !self.autopilot.active {
            self.ship.raw_controls = false;
        }
        n
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
