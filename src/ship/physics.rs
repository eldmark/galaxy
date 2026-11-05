//! Física de la nave a paso fijo: empuje suavizado, gravedad, marcos de referencia, colisión y aterrizaje.
//!
//! Rúbrica:
//! - Tiempo y no cuadros: `step` siempre recibe un `dt` constante (ver `FixedStepper`); todo usa `dt`.
//! - Progresivo: la entrada fija una aceleración/velocidad angular OBJETIVO y el estado la alcanza con
//!   `x += (obj - x) * (1 - e^(-dt/τ))`, igual en todos los equipos.
//! - Gravedad: a = GM/(r²+ε²) hacia cada cuerpo dentro de su esfera de influencia (`Body::gravity_at`).
//! - Colisión: esfera de la nave contra `surface_height_at` (relieve real, marco del cuerpo en el instante t); se reposiciona y se anula la velocidad normal.
//! - Control local: en Orbit/Descent se integra en el marco del planeta (la nave hereda su movimiento).
use super::modes::*;
use super::{Input, Ship};
use crate::math::{DVec3, Quat, Vec3};
use crate::world::{altitude, cross, rot_y, smoothstep01, surface_height_at, terrain_altitude, Body};

pub fn q_rot(q: Quat, v: DVec3) -> DVec3 {
    q.rotate(v.to_f32()).to_f64()
}
pub fn q_rot_inv(q: Quat, v: DVec3) -> DVec3 {
    q.conjugate().rotate(v.to_f32()).to_f64()
}
fn expk(dt: f64, tau: f64) -> f64 {
    1.0 - (-dt / tau).exp()
}

/// Cuerpos que atraen (el sol no: gm = 0) y que sirven de marco.
fn is_planet(b: &Body) -> bool {
    b.gm > 0.0
}

/// Índice del planeta con menor altitud/radio y esa razón. None si no hay planetas.
pub fn nearest_planet(bodies: &[Body], pos: DVec3, t: f64) -> Option<(usize, f64)> {
    let mut best: Option<(usize, f64)> = None;
    for (i, b) in bodies.iter().enumerate() {
        if !is_planet(b) {
            continue;
        }
        let r = altitude(b.position(t), b.radius, pos) / b.radius;
        if best.map_or(true, |(_, br)| r < br) {
            best = Some((i, r));
        }
    }
    best
}

/// Mezcla de co-rotación con el suelo: 0 a 1R de altitud, 1 a 0.5R (suave, sin saltos de velocidad).
pub fn corotation_blend(alt_over_r: f64) -> f64 {
    smoothstep01((1.0 - alt_over_r) / 0.5)
}

/// Velocidad del marco de referencia en el punto relativo `rel` del cuerpo `b`.
fn frame_velocity(b: &Body, t: f64, rel: DVec3, corot: f64) -> DVec3 {
    b.velocity(t) + b.spin_velocity(rel) * corot
}

/// Aceleración gravitatoria bruta en `pos` (suma de cuerpos con GM).
pub fn raw_gravity(bodies: &[Body], pos: DVec3, t: f64) -> DVec3 {
    let mut g = DVec3::ZERO;
    for b in bodies {
        g += b.gravity_at(b.position(t), pos);
    }
    g
}

/// Aceleración gravitatoria efectiva sobre la nave (con interruptor y alivio de asistencia).
pub fn effective_gravity(s: &Ship, bodies: &[Body], t: f64) -> DVec3 {
    if !s.gravity_enabled || s.mode == NavMode::Landed {
        return DVec3::ZERO;
    }
    let g = raw_gravity(bodies, s.position, t);
    let relief = if s.flight_assist {
        match s.mode {
            NavMode::Orbit => s.gravity_relief,
            NavMode::Descent => 1.0 - (1.0 - s.gravity_relief) * 0.5,
            _ => 0.0,
        }
    } else {
        0.0
    };
    g * (1.0 - relief)
}

fn update_mode(s: &mut Ship, t: f64, input: &Input, bodies: &[Body]) {
    let old = s.mode;
    match s.mode {
        NavMode::Landed => {
            let takeoff = input.thrust.y > 0.1 || (s.raw_controls && input.thrust.length() > 0.05);
            if takeoff {
                s.mode = NavMode::Descent;
                s.takeoff_cooldown = 2.0;
                s.accel = DVec3::ZERO;
            }
        }
        NavMode::Cruise => {
            if let Some((i, r)) = nearest_planet(bodies, s.position, t) {
                if r < ORBIT_ENTER {
                    s.mode = NavMode::Orbit;
                    s.frame = Some(i);
                }
            }
        }
        NavMode::Orbit => {
            let b = &bodies[s.frame.unwrap_or(0)];
            let r = altitude(b.position(t), b.radius, s.position) / b.radius;
            if r > ORBIT_EXIT {
                s.mode = NavMode::Cruise;
                s.frame = None;
                s.alt_target = None;
            } else if r < DESCENT_ENTER {
                s.mode = NavMode::Descent;
            }
        }
        NavMode::Descent => {
            let b = &bodies[s.frame.unwrap_or(0)];
            let r = altitude(b.position(t), b.radius, s.position) / b.radius;
            if r > DESCENT_EXIT {
                s.mode = NavMode::Orbit;
            }
        }
    }
    if s.mode != old {
        s.mode_time = 0.0;
    }
}

/// Giro: la entrada fija una velocidad angular objetivo que se alcanza con constante de tiempo TAU_ANGULAR.
fn step_rotation(s: &mut Ship, dt: f64, input: &Input) {
    let target = Vec3::new(
        (input.rotate.x as f64 * RATE_PITCH) as f32,
        (-(input.rotate.y as f64) * RATE_YAW) as f32,
        (-(input.rotate.z as f64) * RATE_ROLL) as f32,
    );
    let k = expk(dt, TAU_ANGULAR) as f32;
    s.angular_velocity = s.angular_velocity + (target - s.angular_velocity) * k;
    let w = s.angular_velocity;
    let ang = w.length() as f64 * dt;
    if ang > 1e-9 {
        let dq = Quat::from_axis_angle(w, ang as f32);
        s.orientation = (s.orientation * dq).normalized(); // giro en ejes locales; renormaliza
    } else {
        s.orientation = s.orientation.normalized();
    }
}

/// Rota la orientación el ángulo mínimo que lleva el "arriba" de la nave hacia `n`, una fracción `k`.
fn level_toward(s: &mut Ship, n: DVec3, k: f64) {
    let up = s.orientation.rotate(Vec3::Y);
    let nf = n.to_f32();
    let axis = up.cross(nf);
    let sin_a = axis.length();
    let cos_a = up.dot(nf);
    let ang = sin_a.atan2(cos_a);
    if ang > 1e-6 && sin_a > 1e-9 {
        let rot = Quat::from_axis_angle(axis, ang * k as f32);
        s.orientation = (rot * s.orientation).normalized();
    }
}

fn target_accel(s: &Ship, input: &Input, rel: Option<DVec3>) -> DVec3 {
    if input.brake {
        return DVec3::ZERO;
    }
    let k = if input.boost { BOOST_ACCEL } else { 1.0 };
    let lx = input.thrust.x as f64 * ACCEL_LAT * k;
    let ly = input.thrust.y as f64 * ACCEL_VERT * k;
    let lz = input.thrust.z as f64 * ACCEL_FWD * k;
    let local_mode = !s.raw_controls && matches!(s.mode, NavMode::Orbit | NavMode::Descent);
    match (local_mode, rel) {
        (true, Some(rel)) if rel.length() > 1.0 => {
            // Marco local: strafe/adelante = horizontales (tangentes a la esfera), vertical = radial.
            let n = rel.normalized();
            let mut fwd = q_rot(s.orientation, DVec3::new(0.0, 0.0, -1.0));
            fwd = fwd - n * fwd.dot(n);
            if fwd.length() < 1e-3 {
                let up = q_rot(s.orientation, DVec3::new(0.0, 1.0, 0.0));
                fwd = up - n * up.dot(n);
            }
            let fwd = fwd.normalized();
            let right = cross(fwd, n).normalized();
            right * lx + n * ly + fwd * lz
        }
        _ => q_rot(s.orientation, DVec3::new(lx, ly, -lz)),
    }
}

fn speed_cap(mode: NavMode, boost: bool, alt: f64) -> f64 {
    let base = if boost { BOOST_SPEED } else { CRUISE_SPEED };
    match mode {
        NavMode::Cruise => base,
        NavMode::Orbit => base.min((ORBIT_SPEED_K * alt).max(20.0)),
        NavMode::Descent => base.min((DESCENT_SPEED_K * alt).max(MIN_SPEED_NEAR_GROUND)),
        NavMode::Landed => 0.0,
    }
}

pub fn step(s: &mut Ship, dt: f64, t: f64, input: &Input, bodies: &[Body]) {
    let t1 = t + dt;
    s.contact = None;
    s.mode_time += dt;
    s.takeoff_cooldown = (s.takeoff_cooldown - dt).max(0.0);
    update_mode(s, t, input, bodies);

    if s.mode == NavMode::Landed {
        step_landed(s, dt, t1, bodies);
        update_gear(s, dt);
        return;
    }
    step_rotation(s, dt, input);

    // --- marco de referencia ---
    let fb = s.frame.filter(|_| s.mode != NavMode::Cruise).map(|i| &bodies[i]);
    let pb0 = fb.map(|b| b.position(t)).unwrap_or(DVec3::ZERO);
    let rel0 = s.position - pb0;
    let alt = fb.map(|b| rel0.length() - b.radius).unwrap_or(f64::MAX);
    let corot = fb.map(|b| corotation_blend(alt / b.radius)).unwrap_or(0.0);
    let fv0 = fb.map(|b| frame_velocity(b, t, rel0, corot)).unwrap_or(DVec3::ZERO);
    let mut u = s.velocity - fv0;

    // --- empuje progresivo: aceleración objetivo -> suavizado exponencial ---
    let a_target = target_accel(s, input, fb.map(|_| rel0));
    s.accel = s.accel + (a_target - s.accel) * expk(dt, TAU_ACCEL);

    // --- gravedad ---
    let g = effective_gravity(s, bodies, t);
    s.last_gravity = g;

    // --- ayuda de ORBITA: mantener altitud al circular (centrípeta + gravedad compensadas) ---
    let mut a_hold = DVec3::ZERO;
    if let (Some(b), NavMode::Orbit) = (fb, s.mode) {
        if s.flight_assist && s.hold_altitude && !s.raw_controls && !input.brake {
            if input.thrust.y.abs() > 0.05 {
                s.alt_target = None;
            } else {
                let h_t = *s.alt_target.get_or_insert(alt);
                let n = rel0.normalized();
                let vr = u.dot(n);
                let vt2 = (u.dot(u) - vr * vr).max(0.0);
                let r = rel0.length();
                let ar = -vt2 / r + 0.5 * (h_t - alt) - 1.5 * vr;
                let a_n = (ar - g.dot(n) - s.accel.dot(n)).clamp(-300.0, 300.0);
                a_hold = n * a_n;
                let _ = b;
            }
        }
    } else {
        s.alt_target = None;
    }

    u += (s.accel + g + a_hold) * dt;

    // --- asistencia de vuelo / freno: amortigua la velocidad respecto del cuerpo de referencia ---
    let has_thrust = input.thrust.length() > 0.02;
    let u_ref = if fb.is_some() {
        DVec3::ZERO
    } else {
        match nearest_planet(bodies, s.position, t) {
            Some((i, r)) if r < 12.0 => bodies[i].velocity(t),
            _ => DVec3::ZERO,
        }
    };
    let damp = if input.brake {
        Some(BRAKE_RATE)
    } else if s.flight_assist && !has_thrust && !s.raw_controls {
        Some(ASSIST_RATE)
    } else {
        None
    };
    if let Some(rate) = damp {
        u = u - (u - u_ref) * expk(dt, 1.0 / rate);
    }

    // --- límite de velocidad (crucero fijo; en local proporcional a la altitud) ---
    let cap = speed_cap(s.mode, input.boost, alt);
    let sp = u.length();
    if sp > cap {
        // tope firme pero sin tirones: el exceso se recorta con desaceleración acotada (CAP_DECEL)
        if s.mode == NavMode::Cruise {
            let new_sp = (sp - CAP_DECEL * dt).max(cap);
            u = u * (new_sp / sp);
        } else {
            // en local (Orbit/Descent) se mantiene el tope suave original: el aterrizaje depende de él
            let f = cap / sp;
            u = u * (1.0 - (1.0 - f) * expk(dt, 1.5));
        }
    }

    // --- integración (semi-implícita) en el marco ---
    if let Some(b) = fb {
        let mut rel = rel0 + u * dt;
        rel = rot_y(rel, corot * b.spin_rate() * dt);
        let fv1 = frame_velocity(b, t1, rel, corot);
        s.position = b.position(t1) + rel;
        s.velocity = u + fv1;
    } else {
        s.position = s.position + u * dt;
        s.velocity = u;
    }

    // --- horizonte estabilizado en DESCENSO ---
    if s.mode == NavMode::Descent && !s.raw_controls {
        if let Some(b) = fb {
            let n = (s.position - b.position(t1)).normalized();
            level_toward(s, n, expk(dt, 1.5));
        }
    }

    let landed = collide(s, dt, t1, bodies);
    if landed {
        s.mode = NavMode::Landed;
        s.mode_time = 0.0;
        s.accel = DVec3::ZERO;
        s.angular_velocity = Vec3::ZERO;
    }

    // altitud de referencia para HUD
    s.altitude = match nearest_planet(bodies, s.position, t1) {
        Some((i, _)) => terrain_altitude(&bodies[i], t1, s.position),
        None => f64::MAX,
    };
    update_gear(s, dt);
}

/// Colisión esfera-nave vs superficie de cada cuerpo. Devuelve true si se posó suavemente.
fn collide(s: &mut Ship, dt: f64, t1: f64, bodies: &[Body]) -> bool {
    let mut landed = false;
    for (i, b) in bodies.iter().enumerate() {
        let pb = b.position(t1);
        let rel = s.position - pb;
        let dist = rel.length();
        let n = if dist > 1e-6 { rel * (1.0 / dist) } else { DVec3::new(0.0, 1.0, 0.0) };
        let min_d = surface_height_at(b, t1, n) + SHIP_RADIUS;
        if dist >= min_d {
            continue;
        }
        // velocidad relativa al suelo (que se mueve: órbita + giro propio)
        let ground = b.velocity(t1) + b.spin_velocity(n * min_d);
        let mut vg = s.velocity - ground;
        let vn = vg.dot(n);
        let impact = (-vn).max(0.0);
        let vt = vg - n * vn;
        let ht = vt.length();
        let up = q_rot(s.orientation, DVec3::new(0.0, 1.0, 0.0));
        let tilt = up.dot(n).clamp(-1.0, 1.0).acos().to_degrees();
        // reposicionar en el contacto y quitar la componente normal (con amortiguamiento)
        s.position = pb + n * min_d;
        if vn < 0.0 {
            vg = vg - n * (vn * (1.0 + RESTITUTION));
        }
        let vt2 = vg - n * vg.dot(n);
        vg = vg - vt2 * (3.0 * dt).min(1.0); // fricción con el suelo
        s.velocity = vg + ground;
        s.contact = Some(i);
        s.contact_speed = impact;
        if b.landable
            && matches!(s.mode, NavMode::Orbit | NavMode::Descent)
            && s.takeoff_cooldown <= 0.0
            && impact < LAND_MAX_VSPEED
            && ht < LAND_MAX_HSPEED
            && tilt < LAND_MAX_TILT_DEG
        {
            s.frame = Some(i);
            s.landed_local = b.world_to_fixed(t1, s.position - pb);
            landed = true;
        }
    }
    landed
}

/// Posada: la nave queda fija al marco superficial (gira con el planeta) y se alinea a la normal.
fn step_landed(s: &mut Ship, dt: f64, t1: f64, bodies: &[Body]) {
    let Some(f) = s.frame else { return };
    let b = &bodies[f];
    let rel = b.fixed_to_world(t1, s.landed_local);
    s.position = b.position(t1) + rel;
    s.velocity = b.velocity(t1) + b.spin_velocity(rel);
    // la orientación acompaña al giro del planeta
    s.orientation = (Quat::from_axis_angle(Vec3::Y, (b.spin_rate() * dt) as f32) * s.orientation).normalized();
    level_toward(s, rel.normalized(), expk(dt, 0.8));
    s.angular_velocity = Vec3::ZERO;
    s.accel = DVec3::ZERO;
    s.altitude = terrain_altitude(b, t1, s.position);
}

fn update_gear(s: &mut Ship, dt: f64) {
    let want = s.mode == NavMode::Landed || (s.mode == NavMode::Descent && s.altitude < 250.0);
    let target = if want { 1.0 } else { 0.0 };
    let step = (0.5 * dt) as f32;
    let d = target - s.gear;
    s.gear += d.clamp(-step, step);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::solar_system;

    fn setup() -> (Vec<Body>, Ship) {
        let bodies = solar_system();
        // 3 radios de Verdalia hacia fuera del sol en t=0 (altitud 2R)
        let b = &bodies[2];
        let p = b.position(0.0);
        let dir = p.normalized();
        let mut s = Ship::new(p + dir * (b.radius * 3.0));
        s.flight_assist = false;
        s.velocity = b.velocity(0.0);
        (bodies, s)
    }

    fn run(s: &mut Ship, bodies: &[Body], inp: &Input, secs: f64) -> f64 {
        let dt = 1.0 / 120.0;
        let n = (secs / dt).round() as usize;
        let mut t = 0.0;
        for _ in 0..n {
            s.step(dt, t, inp, bodies);
            t += dt;
        }
        t
    }

    #[test]
    fn acceleration_is_progressive() {
        let bodies = solar_system();
        let mut s = Ship::new(DVec3::new(0.0, 0.0, 60000.0));
        let inp = Input { thrust: Vec3::new(0.0, 0.0, 1.0), ..Default::default() };
        let mut prev = s.velocity.length();
        let mut t = 0.0;
        let mut max_jump: f64 = 0.0;
        for k in 0..600 {
            s.step(1.0 / 120.0, t, &inp, &bodies);
            t += 1.0 / 120.0;
            let v = s.velocity.length();
            max_jump = max_jump.max((v - prev).abs());
            prev = v;
            if k == 2 {
                assert!(v < 5.0, "no debe saltar: v={v}");
            }
        }
        assert!(max_jump < 400.0 / 120.0 * 1.01, "salto por paso {max_jump}");
        assert!(prev > 300.0);
    }

    #[test]
    fn gravity_pulls_and_bends_trajectory() {
        let (bodies, mut a) = setup();
        let mut b = a.clone();
        a.gravity_enabled = true;
        b.gravity_enabled = false;
        // velocidad lateral respecto del planeta para ver la curvatura
        let radial = (a.position - bodies[2].position(0.0)).normalized();
        let side = cross(radial, DVec3::new(0.0, 1.0, 0.0)).normalized() * 120.0;
        a.velocity = a.velocity + side;
        b.velocity = b.velocity + side;
        let inp = Input::default();
        let d0 = (a.position - bodies[2].position(0.0)).length();
        let t = run(&mut a, &bodies, &inp, 8.0);
        run(&mut b, &bodies, &inp, 8.0);
        let d1 = (a.position - bodies[2].position(t)).length();
        let d1n = (b.position - bodies[2].position(t)).length();
        assert!(d1 < d0, "debe acercarse: {d0} -> {d1}");
        assert!(d1 < d1n);
        // la trayectoria se curva: las posiciones difieren notablemente
        assert!((a.position - b.position).length() > 100.0);
    }

    #[test]
    fn never_inside_planet() {
        let bodies = solar_system();
        for (k, idx) in [1usize, 2, 3, 5].iter().enumerate() {
            let b = &bodies[*idx];
            let p = b.position(0.0);
            let mut s = Ship::new(p + DVec3::new(b.radius * 2.5, b.radius * 0.5 * k as f64, 0.0));
            s.velocity = b.velocity(0.0);
            let inp = Input { thrust: Vec3::new(-0.5, 0.0, 0.0), boost: true, ..Default::default() };
            // lanzar contra el planeta: apuntar con la nave y empujar
            let mut t = 0.0;
            for step in 0..6000 {
                // empuje hacia el centro del planeta (marco global, via autopilot-like raw)
                s.raw_controls = true;
                let to = (b.position(t) - s.position).normalized();
                let loc = q_rot_inv(s.orientation, to);
                let i2 = Input {
                    thrust: Vec3::new((loc.x) as f32, (loc.y) as f32, (-loc.z) as f32),
                    boost: inp.boost,
                    ..Default::default()
                };
                s.step(1.0 / 120.0, t, &i2, &bodies);
                t += 1.0 / 120.0;
                for bb in &bodies {
                    // altitud sobre el terreno real (con relieve), no sobre la esfera lisa
                    let d = terrain_altitude(bb, t, s.position);
                    assert!(d >= SHIP_RADIUS - 1e-6, "dentro de {} paso {step}: {d}", bb.name);
                }
            }
        }
    }

    #[test]
    fn orbit_mode_hysteresis_does_not_chatter() {
        let bodies = solar_system();
        let b = &bodies[2];
        let p = b.position(0.0);
        // justo en el umbral de salida (3.5R): deriva lenta hacia afuera y adentro
        let mut s = Ship::new(p + DVec3::new(0.0, 0.0, b.radius * 4.0));
        s.velocity = b.velocity(0.0);
        s.gravity_enabled = false;
        let mut t = 0.0;
        let mut changes = 0;
        let mut last = s.mode;
        for k in 0..4000 {
            // oscila la altitud alrededor de 3.25R (entre 3.0 y 3.5)
            let ph = (k as f64 / 4000.0) * 6.0 * std::f64::consts::PI;
            let alt_r = 3.25 + 0.2 * ph.sin();
            let rel = DVec3::new(0.0, 0.0, b.radius * (1.0 + alt_r));
            let pos = b.position(t) + rel;
            s.position = pos;
            s.velocity = b.velocity(t);
            s.step(1.0 / 120.0, t, &Input::default(), &bodies);
            t += 1.0 / 120.0;
            if s.mode != last {
                changes += 1;
                last = s.mode;
            }
        }
        assert!(changes <= 1, "oscila de modo: {changes}");
        // cruzar el umbral una vez de entrada y una de salida: máx 2 cambios
        let mut s = Ship::new(p + DVec3::new(0.0, 0.0, b.radius * 4.5));
        s.gravity_enabled = false;
        let mut changes = 0;
        let mut last = s.mode;
        let mut t = 0.0;
        for k in 0..4000 {
            let alt_r = 4.5 - 2.0 * ((k as f64 / 4000.0) * 3.0 * std::f64::consts::PI).sin().abs();
            s.position = b.position(t) + DVec3::new(0.0, 0.0, b.radius * (1.0 + alt_r));
            s.step(1.0 / 120.0, t, &Input::default(), &bodies);
            t += 1.0 / 120.0;
            if s.mode != last {
                changes += 1;
                last = s.mode;
            }
        }
        assert!(changes <= 6, "cambios {changes}");
    }

    #[test]
    fn landing_and_takeoff_manual() {
        let bodies = solar_system();
        let b = &bodies[1];
        let t0 = 0.0;
        let rel = DVec3::new(0.0, b.radius + 60.0, 0.0);
        let mut s = Ship::new(b.position(t0) + rel);
        s.velocity = b.velocity(t0) + b.spin_velocity(rel);
        s.mode = NavMode::Descent;
        s.frame = Some(1);
        // orientar "arriba" = radial = +Y ya es la identidad
        let mut t = t0;
        for _ in 0..(60 * 120) {
            s.step(1.0 / 120.0, t, &Input::default(), &bodies);
            t += 1.0 / 120.0;
            if s.mode == NavMode::Landed {
                break;
            }
        }
        assert_eq!(s.mode, NavMode::Landed);
        let up = Input { thrust: Vec3::new(0.0, 1.0, 0.0), ..Default::default() };
        for _ in 0..(5 * 120) {
            s.step(1.0 / 120.0, t, &up, &bodies);
            t += 1.0 / 120.0;
        }
        assert_ne!(s.mode, NavMode::Landed);
        assert!(s.altitude > 100.0, "alt {}", s.altitude);
    }
}
