//! Autopiloto (Tour): máquina de estados que produce un `Input` SINTÉTICO, de modo que comparte con el
//! jugador la misma física, límites y colisión (rúbrica: no es un "teletransporte" de la nave).
//!
//! Fases: Align -> Transit (perfil trapezoidal con intercepción) -> Approach -> Orbit (una vuelta)
//! -> Descend (espiral con ease_in_out) -> Land (flare, tren) -> Wait (5 s) -> Takeoff -> siguiente.
//! Todo es determinista: función del tiempo de simulación y del estado de la nave.
use super::modes::*;
use super::physics::{corotation_blend, effective_gravity, q_rot, q_rot_inv};
use super::{Input, NavMode, Ship};
use crate::math::{DVec3, Quat, Vec3};
use crate::world::{altitude, cross, smoothstep01, surface_height, Body, TAU};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Align,
    Transit,
    Approach,
    Orbit,
    Descend,
    Land,
    Wait,
    Takeoff,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Idle => "INACTIVO",
            Phase::Align => "ALINEAR",
            Phase::Transit => "TRANSITO",
            Phase::Approach => "APROXIMAR",
            Phase::Orbit => "ORBITAR",
            Phase::Descend => "DESCENDER",
            Phase::Land => "POSAR",
            Phase::Wait => "ESPERAR",
            Phase::Takeoff => "DESPEGAR",
        }
    }
}

// ---- Constantes del autopiloto ----
const V_CRUISE: f64 = 1600.0; // velocidad de crucero relativa al destino (con boost el tope es 2000 m/s)
const A_DEC: f64 = 250.0; // deceleración de diseño del perfil (m/s²) < aceleración disponible
const A_LIM: f64 = 500.0; // tope de aceleración pedida
const APPROACH_R: f64 = 3.5; // radios (centro) del punto de espera
const ORBIT_R: f64 = 1.7; // radio de la órbita (altitud 1R)
const A_PROF: f64 = 220.0; // aceleración de diseño de los perfiles suaves (Approach)
const ORBIT_SPEED: f64 = 1200.0;
const ORBIT_RAMP: f64 = 4.0;
const FLARE_ALT: f64 = 60.0; // últimos 60 m
const WAIT_T: f64 = 5.0;
const KP: f64 = 0.64;
const KD: f64 = 1.6;

fn clamp_mag(v: DVec3, m: f64) -> DVec3 {
    let l = v.length();
    if l > m { v * (m / l) } else { v }
}

fn rodrigues(v: DVec3, k: DVec3, a: f64) -> DVec3 {
    let (s, c) = a.sin_cos();
    v * c + cross(k, v) * s + k * (k.dot(v) * (1.0 - c))
}

fn slerp(a: DVec3, b: DVec3, e: f64) -> DVec3 {
    let d = a.dot(b).clamp(-1.0, 1.0);
    let ang = d.acos();
    if ang < 1e-6 {
        return a;
    }
    let s = ang.sin();
    (a * (((1.0 - e) * ang).sin() / s) + b * ((e * ang).sin() / s)).normalized()
}

fn quat_from_basis(r: DVec3, u: DVec3, b: DVec3) -> Quat {
    let (m00, m01, m02) = (r.x, u.x, b.x);
    let (m10, m11, m12) = (r.y, u.y, b.y);
    let (m20, m21, m22) = (r.z, u.z, b.z);
    let tr = m00 + m11 + m22;
    let (x, y, z, w);
    if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        w = 0.25 * s;
        x = (m21 - m12) / s;
        y = (m02 - m20) / s;
        z = (m10 - m01) / s;
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        w = (m21 - m12) / s;
        x = 0.25 * s;
        y = (m01 + m10) / s;
        z = (m02 + m20) / s;
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        w = (m02 - m20) / s;
        x = (m01 + m10) / s;
        y = 0.25 * s;
        z = (m12 + m21) / s;
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        w = (m10 - m01) / s;
        x = (m02 + m20) / s;
        y = (m12 + m21) / s;
        z = 0.25 * s;
    }
    Quat { x: x as f32, y: y as f32, z: z as f32, w: w as f32 }.normalized()
}

/// Control PD de orientación: devuelve el `rotate` que lleva el morro a `fwd` con "arriba" ≈ `up`.
fn orient_input(q: Quat, fwd: DVec3, up: DVec3) -> Vec3 {
    let f = fwd.normalized();
    if f.length() < 0.5 {
        return Vec3::ZERO;
    }
    let mut r = cross(f, up);
    if r.length() < 1e-4 {
        r = cross(f, DVec3::new(1.0, 0.0, 0.0));
        if r.length() < 1e-4 {
            r = cross(f, DVec3::new(0.0, 0.0, 1.0));
        }
    }
    let r = r.normalized();
    let u = cross(r, f);
    let qd = quat_from_basis(r, u, -f);
    let mut e = qd * q.conjugate();
    if e.w < 0.0 {
        e = Quat { x: -e.x, y: -e.y, z: -e.z, w: -e.w };
    }
    let s = (e.x * e.x + e.y * e.y + e.z * e.z).sqrt();
    if s < 1e-7 {
        return Vec3::ZERO;
    }
    let ang = 2.0 * s.atan2(e.w);
    let axis = Vec3::new(e.x / s, e.y / s, e.z / s);
    let w_local = q.conjugate().rotate(axis * (ang * 1.5));
    Vec3::new(
        (w_local.x as f64 / RATE_PITCH).clamp(-1.0, 1.0) as f32,
        (-(w_local.y as f64) / RATE_YAW).clamp(-1.0, 1.0) as f32,
        (-(w_local.z as f64) / RATE_ROLL).clamp(-1.0, 1.0) as f32,
    )
}

/// Convierte una aceleración deseada (mundo) en empuje normalizado en ejes de nave.
fn accel_to_thrust(q: Quat, a: DVec3, boost: bool) -> Vec3 {
    let k = if boost { BOOST_ACCEL } else { 1.0 };
    let l = q_rot_inv(q, a);
    Vec3::new(
        (l.x / (ACCEL_LAT * k)).clamp(-1.0, 1.0) as f32,
        (l.y / (ACCEL_VERT * k)).clamp(-1.0, 1.0) as f32,
        (-l.z / (ACCEL_FWD * k)).clamp(-1.0, 1.0) as f32,
    )
}

#[derive(Clone, Debug)]
pub struct Autopilot {
    pub active: bool,
    /// true = Tour completo; false = solo descenso y posada del cuerpo actual.
    pub tour: bool,
    pub phase: Phase,
    /// Índice del cuerpo destino.
    pub target: usize,
    /// Tren de aterrizaje 0..1 (copia de `Ship::gear`) para que el coordinador anime la malla.
    pub gear: f32,
    /// Cuerpos aterrizados en la ronda actual.
    pub visited: Vec<usize>,
    pub landings: u32,
    pub takeoffs: u32,
    pub cycles: u32,
    /// Sitios de aterrizaje por cuerpo (dirección unitaria fija al cuerpo) que el terreno puede registrar.
    pub landing_sites: Vec<(usize, DVec3)>,

    eta: f64,
    phase_t: f64,
    t0: f64,
    hold_v: DVec3,
    waypoint: Option<DVec3>,
    wp_count: u32,
    e1: DVec3,
    e2: DVec3,
    r0: f64,
    app_total: f64,
    orbit_total: f64,
    d0: DVec3,
    site: DVec3,
    alt0: f64,
    desc_total: f64,
    alt_ref: f64,
    off_dir: DVec3,
}

impl Default for Autopilot {
    fn default() -> Self {
        Self::new()
    }
}

impl Autopilot {
    pub fn new() -> Self {
        Self {
            active: false,
            tour: true,
            phase: Phase::Idle,
            target: 0,
            gear: 0.0,
            visited: Vec::new(),
            landings: 0,
            takeoffs: 0,
            cycles: 0,
            landing_sites: Vec::new(),
            eta: 0.0,
            phase_t: 0.0,
            t0: 0.0,
            hold_v: DVec3::ZERO,
            waypoint: None,
            wp_count: 0,
            e1: DVec3::ZERO,
            e2: DVec3::ZERO,
            r0: 0.0,
            app_total: 8.0,
            orbit_total: 1.0,
            d0: DVec3::new(0.0, 1.0, 0.0),
            site: DVec3::new(0.0, 1.0, 0.0),
            alt0: 0.0,
            desc_total: 1.0,
            alt_ref: 0.0,
            off_dir: DVec3::new(1.0, 0.0, 0.0),
        }
    }

    // ---------- API para el HUD / coordinador ----------
    /// Fase actual como texto para el HUD.
    pub fn status(&self) -> &'static str {
        if self.active { self.phase.name() } else { "INACTIVO" }
    }
    /// ETA (s) de la fase actual o del destino (Transit).
    pub fn eta(&self) -> f64 {
        self.eta.max(0.0)
    }
    pub fn target_name<'a>(&self, bodies: &'a [Body]) -> &'a str {
        bodies.get(self.target).map(|b| b.name).unwrap_or("-")
    }

    /// Inicia el Tour desde el estado actual de la nave.
    pub fn start(&mut self, ship: &Ship, bodies: &[Body], t: f64) {
        self.active = true;
        self.tour = true;
        let exclude = if ship.mode == NavMode::Landed { ship.frame } else { None };
        self.target = self.pick_next(ship.position, bodies, t, exclude);
        if ship.mode == NavMode::Landed {
            self.set_phase(Phase::Takeoff, t, ship, bodies);
        } else if ship.mode != NavMode::Cruise && ship.frame == Some(self.target) {
            self.set_phase(Phase::Approach, t, ship, bodies);
        } else {
            self.set_phase(Phase::Align, t, ship, bodies);
        }
    }

    /// Detiene el autopiloto; la nave conserva su estado (sin saltos).
    pub fn stop(&mut self) {
        self.active = false;
        self.phase = Phase::Idle;
    }

    /// Solo descenso y posada en el cuerpo aterrizable más cercano (debe estar en Orbit/Descent).
    pub fn start_landing_here(&mut self, ship: &Ship, bodies: &[Body], t: f64) -> bool {
        if ship.mode == NavMode::Landed || ship.mode == NavMode::Cruise {
            return false;
        }
        let mut best: Option<(usize, f64)> = None;
        for (i, b) in bodies.iter().enumerate() {
            if !b.landable {
                continue;
            }
            let r = altitude(b.position(t), b.radius, ship.position) / b.radius;
            if r < ORBIT_EXIT && best.map_or(true, |(_, br)| r < br) {
                best = Some((i, r));
            }
        }
        let Some((i, _)) = best else { return false };
        self.active = true;
        self.tour = false;
        self.target = i;
        self.set_phase(Phase::Descend, t, ship, bodies);
        true
    }

    fn pick_next(&mut self, pos: DVec3, bodies: &[Body], t: f64, exclude: Option<usize>) -> usize {
        let mut cand: Vec<usize> = (0..bodies.len())
            .filter(|&i| bodies[i].landable && !self.visited.contains(&i) && Some(i) != exclude)
            .collect();
        if cand.is_empty() {
            self.visited.clear();
            self.cycles += 1;
            cand = (0..bodies.len()).filter(|&i| bodies[i].landable && Some(i) != exclude).collect();
        }
        let mut best = cand.first().copied().unwrap_or(0);
        let mut bd = f64::MAX;
        for &i in &cand {
            let d = (bodies[i].position(t) - pos).length();
            if d < bd {
                bd = d;
                best = i;
            }
        }
        best
    }

    fn site_for(&self, idx: usize, d0: DVec3) -> DVec3 {
        if let Some((_, s)) = self.landing_sites.iter().find(|(i, _)| *i == idx) {
            return s.normalized();
        }
        // sitio por defecto: un poco "adelante" de la entrada, sobre un gran círculo
        let mut k = cross(d0, DVec3::new(0.0, 1.0, 0.0));
        if k.length() < 1e-3 {
            k = DVec3::new(1.0, 0.0, 0.0);
        }
        rodrigues(d0, k.normalized(), 0.3)
    }

    fn set_phase(&mut self, ph: Phase, t: f64, ship: &Ship, bodies: &[Body]) {
        self.phase = ph;
        self.phase_t = 0.0;
        self.t0 = t;
        self.hold_v = ship.velocity;
        let b = &bodies[self.target];
        let rel = ship.position - b.position(t);
        match ph {
            Phase::Align => {
                self.waypoint = None;
                self.wp_count = 0;
                // lado del planeta por el que se llega: fijo durante todo el tránsito (objetivo estable)
                self.off_dir = rel.normalized();
            }
            Phase::Transit => {}
            Phase::Approach => {
                self.e1 = rel.normalized();
                self.r0 = rel.length();
                let delta = (self.r0 - b.radius * ORBIT_R).abs();
                self.app_total = (6.0 * delta / A_PROF).sqrt().max(5.0);
            }
            Phase::Orbit => {
                self.e1 = rel.normalized();
                let a = if self.e1.y.abs() > 0.9 { DVec3::new(1.0, 0.0, 0.0) } else { DVec3::new(0.0, 1.0, 0.0) };
                let n = cross(self.e1, a).normalized();
                self.e2 = cross(n, self.e1).normalized();
                self.r0 = rel.length();
                let sp = ORBIT_SPEED.min((280.0 * self.r0).sqrt()); // centrípeta v²/r acotada
                self.orbit_total = ORBIT_RAMP + TAU * self.r0 / sp;
            }
            Phase::Descend => {
                let d_world = rel.normalized();
                self.d0 = b.world_to_fixed(t, d_world);
                self.site = self.site_for(self.target, self.d0);
                self.alt0 = rel.length() - surface_height(b, d_world);
                self.desc_total = (10.0 + self.alt0 / 300.0).clamp(12.0, 40.0);
                if self.alt0 < 2.0 * FLARE_ALT {
                    self.phase = Phase::Land;
                    self.alt_ref = self.alt0;
                    self.site = self.d0;
                }
            }
            Phase::Land => {
                self.alt_ref = rel.length() - surface_height(b, rel.normalized());
            }
            _ => {}
        }
    }

    /// Punto de intercepción: posición del planeta en `t + ETA` más un desvío hacia el lado de la nave.
    /// Itera 4 veces (la órbita es analítica, es barato y estable).
    fn intercept(&self, p: DVec3, t: f64, bodies: &[Body]) -> (DVec3, f64) {
        let b = &bodies[self.target];
        let off = self.off_dir * (b.radius * APPROACH_R);
        let mut eta = 0.0;
        let mut tp = b.position(t) + off;
        for _ in 0..4 {
            tp = b.position(t + eta) + off;
            let d = (tp - p).length();
            eta = d / V_CRUISE + V_CRUISE / A_DEC;
        }
        (tp, eta)
    }

    /// Aceleración de seguimiento de una referencia p_ref(t) (con feed-forward de v y a, y cancelando la gravedad).
    fn track<F: Fn(f64) -> DVec3>(&self, ship: &Ship, bodies: &[Body], t: f64, pref: F) -> DVec3 {
        let h = 0.1;
        let (pm, p, pp) = (pref(t - h), pref(t), pref(t + h));
        let vref = (pp - pm) * (1.0 / (2.0 * h));
        let aref = (pp - p * 2.0 + pm) * (1.0 / (h * h));
        let g = effective_gravity(ship, bodies, t);
        aref + (p - ship.position) * KP + (vref - ship.velocity) * KD - g
    }

    fn input_from(&self, ship: &Ship, a: DVec3, rotate: Vec3, boost: bool) -> Input {
        let a = clamp_mag(a, A_LIM);
        Input { thrust: accel_to_thrust(ship.orientation, a, boost), rotate, boost, brake: false }
    }

    fn radial_up(&self, ship: &Ship, b: &Body, t: f64) -> DVec3 {
        (ship.position - b.position(t)).normalized()
    }

    /// Un paso de control: devuelve el `Input` sintético. Llamar en cada paso fijo (antes de `Ship::step`).
    pub fn update(&mut self, dt: f64, t: f64, ship: &Ship, bodies: &[Body]) -> Input {
        if !self.active {
            return Input::default();
        }
        self.phase_t += dt;
        self.gear = ship.gear;
        let tgt = self.target;
        let b = &bodies[tgt];
        let up_y = DVec3::new(0.0, 1.0, 0.0);
        let g = effective_gravity(ship, bodies, t);
        let fwd_now = q_rot(ship.orientation, DVec3::new(0.0, 0.0, -1.0));
        match self.phase {
            Phase::Idle => Input::default(),
            Phase::Align => {
                let (tp, eta) = self.intercept(ship.position, t, bodies);
                self.eta = eta + 2.0;
                let aim = (tp - ship.position).normalized();
                // mantener la velocidad del marco actual mientras gira
                let vhold = match (ship.mode, ship.frame) {
                    (NavMode::Cruise, _) | (_, None) => self.hold_v,
                    (_, Some(f)) => bodies[f].velocity(t),
                };
                let a = (vhold - ship.velocity) * 1.2 - g;
                let rot = orient_input(ship.orientation, aim, up_y);
                let err = fwd_now.dot(aim).clamp(-1.0, 1.0).acos();
                let w = ship.angular_velocity.length();
                if (err < 0.06 && w < 0.1) || self.phase_t > 10.0 {
                    self.set_phase(Phase::Transit, t, ship, bodies);
                }
                self.input_from(ship, a, rot, false)
            }
            Phase::Transit => {
                let (tp, eta) = self.intercept(ship.position, t, bodies);
                self.eta = eta;
                // evitar cuerpos que cruzan la ruta (sol incluido): punto intermedio por el lado
                if self.waypoint.is_none() && self.wp_count < 3 {
                    self.waypoint = self.avoid(ship.position, tp, t, eta, bodies);
                    if self.waypoint.is_some() {
                        self.wp_count += 1;
                    }
                }
                let (aim_pt, via) = match self.waypoint {
                    Some(w) => (w, true),
                    None => (tp, false),
                };
                let to = aim_pt - ship.position;
                let d = to.length();
                let dir = to.normalized();
                if via && d < 2500.0 {
                    self.waypoint = None;
                }
                let (vdes, arrive) = if via {
                    (dir * V_CRUISE, false)
                } else {
                    let v_arr = b.velocity(t + eta);
                    let sp = V_CRUISE.min((2.0 * A_DEC * d).sqrt()).min(0.8 * d);
                    (v_arr + dir * sp, true)
                };
                let a = (vdes - ship.velocity) * 2.0 - g;
                let rot = orient_input(ship.orientation, dir, up_y);
                if arrive && (d < 150.0 || (d < 500.0 && (ship.velocity - b.velocity(t)).length() < 150.0)) {
                    self.set_phase(Phase::Approach, t, ship, bodies);
                }
                self.input_from(ship, a, rot, true)
            }
            Phase::Approach => {
                let (e1, r0) = (self.e1, self.r0);
                let r1 = b.radius * ORBIT_R;
                let t0 = self.t0;
                let tot = self.app_total;
                self.eta = tot - self.phase_t;
                let pref = |tt: f64| {
                    let s = smoothstep01((tt - t0) / tot);
                    b.position(tt) + e1 * (r0 + (r1 - r0) * s)
                };
                let a = self.track(ship, bodies, t, pref);
                let rot = orient_input(ship.orientation, -e1, up_y);
                if self.phase_t >= tot + 0.5 {
                    self.set_phase(Phase::Orbit, t, ship, bodies);
                }
                self.input_from(ship, a, rot, true)
            }
            Phase::Orbit => {
                let (e1, e2, r) = (self.e1, self.e2, self.r0.max(b.radius * 1.2));
                let (t0, total) = (self.t0, self.orbit_total);
                self.eta = total - self.phase_t + 20.0;
                let wmax = TAU / (total - ORBIT_RAMP);
                let phi = move |tau: f64| {
                    let tau = tau.clamp(0.0, total);
                    let tr = ORBIT_RAMP;
                    if tau < tr {
                        wmax * tau * tau / (2.0 * tr)
                    } else if tau < total - tr {
                        wmax * (tr / 2.0 + (tau - tr))
                    } else {
                        let d = total - tau;
                        TAU - wmax * d * d / (2.0 * tr)
                    }
                };
                let pref = |tt: f64| {
                    let f = phi(tt - t0);
                    b.position(tt) + (e1 * f.cos() + e2 * f.sin()) * r
                };
                let a = self.track(ship, bodies, t, pref);
                let f = phi(t - t0);
                let tangent = e2 * f.cos() - e1 * f.sin();
                let outward = e1 * f.cos() + e2 * f.sin();
                let rot = orient_input(ship.orientation, tangent, outward);
                if self.phase_t >= total + 0.5 {
                    self.set_phase(Phase::Descend, t, ship, bodies);
                }
                self.input_from(ship, a, rot, true)
            }
            Phase::Descend => {
                let (d0, site, alt0, total, t0) = (self.d0, self.site, self.alt0, self.desc_total, self.t0);
                self.eta = (total - self.phase_t).max(0.0) + 14.0;
                let pref = |tt: f64| {
                    let s = ((tt - t0) / total).clamp(0.0, 1.0);
                    let e = smoothstep01(s); // ease_in_out
                    let dir = slerp(d0, site, e);
                    let alt = FLARE_ALT + (alt0 - FLARE_ALT) * (1.0 - e).powf(1.5);
                    let wd = b.fixed_to_world(tt, dir);
                    b.position(tt) + wd * (surface_height(b, wd) + alt)
                };
                let a = self.track(ship, bodies, t, pref);
                let n = self.radial_up(ship, b, t);
                let heading = (fwd_now - n * fwd_now.dot(n)).normalized();
                let rot = orient_input(ship.orientation, heading, n);
                if self.phase_t >= total || ship.mode == NavMode::Landed {
                    self.set_phase(Phase::Land, t, ship, bodies);
                    self.alt_ref = self.alt_ref.min(FLARE_ALT + 20.0);
                    self.site = site;
                }
                self.input_from(ship, a, rot, false)
            }
            Phase::Land => {
                let site = self.site;
                // descenso vertical: la velocidad decae hasta ~2 m/s al acercarse al suelo
                let v_down = (3.0 + 0.2 * (self.alt_ref - 8.0)).clamp(3.0, 14.0);
                self.alt_ref = (self.alt_ref - v_down * dt).max(-4.0);
                let (alt_ref, t_now) = (self.alt_ref, t);
                self.eta = (self.alt_ref.max(0.0) / v_down.max(3.0)).min(15.0);
                let pref = move |tt: f64| {
                    let alt = alt_ref - v_down * (tt - t_now);
                    let wd = b.fixed_to_world(tt, site);
                    b.position(tt) + wd * (surface_height(b, wd) + alt)
                };
                let a = self.track(ship, bodies, t, pref);
                let n = self.radial_up(ship, b, t);
                let heading = (fwd_now - n * fwd_now.dot(n)).normalized();
                // flare: el morro se levanta unos grados en los últimos 60 m
                let fl = ((FLARE_ALT - self.alt_ref) / FLARE_ALT).clamp(0.0, 1.0) * 6f64.to_radians();
                let fwd_des = heading * fl.cos() + n * fl.sin();
                let rot = orient_input(ship.orientation, fwd_des, n);
                if ship.mode == NavMode::Landed {
                    self.landings += 1;
                    if !self.visited.contains(&tgt) {
                        self.visited.push(tgt);
                    }
                    if self.tour {
                        self.set_phase(Phase::Wait, t, ship, bodies);
                    } else {
                        self.stop();
                    }
                    return Input::default();
                }
                if self.phase_t > 90.0 {
                    // algo falló: reintentar desde arriba
                    self.set_phase(Phase::Takeoff, t, ship, bodies);
                }
                self.input_from(ship, a, rot, false)
            }
            Phase::Wait => {
                self.eta = (WAIT_T - self.phase_t).max(0.0);
                if self.phase_t >= WAIT_T {
                    self.set_phase(Phase::Takeoff, t, ship, bodies);
                }
                Input::default()
            }
            Phase::Takeoff => {
                let fi = ship.frame.unwrap_or(tgt);
                let fb = &bodies[fi];
                let rel = ship.position - fb.position(t);
                let n = rel.normalized();
                let alt = rel.length() - fb.radius;
                let corot = corotation_blend(alt / fb.radius);
                let vf = fb.velocity(t) + fb.spin_velocity(rel) * corot;
                // empuje vertical con rampa: misma curva de aterrizaje invertida
                let v_up = (15.0 + 50.0 * self.phase_t).min(0.4 * alt.max(0.0) + 10.0).min(500.0).max(15.0);
                let a = (vf + n * v_up - ship.velocity) * 2.0 - g;
                let heading = (fwd_now - n * fwd_now.dot(n)).normalized();
                let rot = orient_input(ship.orientation, heading, n);
                self.eta = ((0.6 * fb.radius - alt) / 150.0).max(0.0) + 2.0;
                if ship.mode != NavMode::Landed && alt > 0.6 * fb.radius {
                    self.takeoffs += 1;
                    // siguiente destino de la ruta
                    if !self.tour {
                        self.stop();
                        return Input::default();
                    }
                    self.target = self.pick_next(ship.position, bodies, t, Some(fi));
                    self.set_phase(Phase::Align, t, ship, bodies);
                }
                self.input_from(ship, a, rot, false)
            }
        }
    }

    /// Si el segmento p->q pasa demasiado cerca de algún cuerpo que no es el destino, devuelve un
    /// punto intermedio que lo rodea por un lado.
    fn avoid(&self, p: DVec3, q: DVec3, t: f64, eta: f64, bodies: &[Body]) -> Option<DVec3> {
        let d = q - p;
        let dd = d.dot(d);
        if dd < 1.0 {
            return None;
        }
        for (i, b) in bodies.iter().enumerate() {
            if i == self.target {
                continue;
            }
            let (limit, way) = if b.gm <= 0.0 { (1.7, 2.2) } else { (2.4, 3.0) };
            let tc = (-(p.dot(d) - b.position(t).dot(d)) / dd).clamp(0.0, 1.0);
            let c = b.position(t + eta * tc);
            let closest = p + d * tc;
            let off = closest - c;
            if off.length() < limit * b.radius {
                let mut dir = off.normalized();
                if off.length() < 1.0 {
                    dir = cross(d, DVec3::new(0.0, 1.0, 0.0)).normalized();
                }
                return Some(c + dir * (way * b.radius));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::Sim;
    use crate::world::solar_system;

    fn spawn_sim() -> Sim {
        let bodies = solar_system();
        let (p, rad) = (bodies[1].position(0.0), bodies[1].radius);
        let mut sim = Sim::new(bodies, p + p.normalized() * (rad * 8.0));
        sim.ship.velocity = sim.bodies[1].velocity(0.0);
        sim
    }

    #[test]
    fn full_tour_visits_all_landable_and_lands() {
        let mut sim = spawn_sim();
        let n_land = sim.bodies.iter().filter(|b| b.landable).count();
        sim.start_tour();
        let mut landed_bodies: Vec<usize> = vec![];
        let mut sun_contacts = 0;
        let mut max_t = 0.0;
        let mut seen_landed = false;
        let mut frame = 0;
        let mut lastph = Phase::Idle;
        loop {
            frame += 1;
            // cuadros irregulares: 30..60 FPS
            let fdt = if frame % 3 == 0 { 1.0 / 30.0 } else { 1.0 / 60.0 };
            sim.frame(fdt, &Input::default());
            if sim.ship.contact == Some(0) {
                sun_contacts += 1;
            }
            if sim.ship.mode == NavMode::Landed {
                seen_landed = true;
                if let Some(f) = sim.ship.frame {
                    if !landed_bodies.contains(&f) {
                        landed_bodies.push(f);
                    }
                }
            }
            max_t = sim.t;
            if std::env::var("TOURDBG").is_ok() && (frame % 60 == 0 || sim.autopilot.phase != lastph) {
                println!("t={:.1} {:?} tgt={} mode={:?} alt={:.0} speed={:.0} pos={:?}", sim.t, sim.autopilot.phase, sim.autopilot.target, sim.ship.mode, sim.ship.altitude, sim.ship.velocity.length(), (sim.ship.position.x as i64, sim.ship.position.y as i64, sim.ship.position.z as i64));
                lastph = sim.autopilot.phase;
            }
            if sim.autopilot.takeoffs as usize >= n_land || sim.t > 9.0 * 60.0 {
                break;
            }
        }
        println!("tour: t={max_t:.1}s landed={landed_bodies:?} takeoffs={}", sim.autopilot.takeoffs);
        assert!(seen_landed);
        assert_eq!(landed_bodies.len(), n_land, "debe aterrizar en todos los landable");
        assert_eq!(sun_contacts, 0, "no debe tocar el sol");
        assert!(max_t < 8.0 * 60.0, "tour demasiado largo: {max_t}");
    }

    #[test]
    fn manual_input_interrupts_without_jump() {
        let mut sim = spawn_sim();
        sim.start_tour();
        for _ in 0..(60 * 20) {
            sim.frame(1.0 / 60.0, &Input::default());
        }
        assert!(sim.autopilot.active);
        let v0 = sim.ship.velocity;
        let p0 = sim.ship.position;
        let inp = Input { thrust: Vec3::new(0.0, 0.0, 1.0), ..Default::default() };
        sim.frame(1.0 / 60.0, &inp);
        assert!(!sim.autopilot.active, "debe cederse el control");
        assert!(!sim.ship.raw_controls);
        assert!((sim.ship.velocity - v0).length() < 15.0, "salto de velocidad");
        assert!((sim.ship.position - p0).length() < 100.0, "salto de posición");
    }

    #[test]
    fn landing_here_lands_on_nearest() {
        let bodies = solar_system();
        let (p, rad) = (bodies[1].position(0.0), bodies[1].radius);
        let mut sim = Sim::new(bodies, p + DVec3::new(0.0, rad * 1.5, 0.0));
        sim.ship.velocity = sim.bodies[1].velocity(0.0);
        sim.ship.mode = NavMode::Orbit;
        sim.ship.frame = Some(1);
        assert!(sim.land_here());
        for _ in 0..(60 * 120) {
            sim.frame(1.0 / 60.0, &Input::default());
            if sim.ship.mode == NavMode::Landed {
                break;
            }
        }
        assert_eq!(sim.ship.mode, NavMode::Landed);
        assert!(!sim.autopilot.active);
    }
}
