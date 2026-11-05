//! Harness headless de "sensación de control": simula vuelo con entradas guionadas (teclas -> `map_keys` ->
//! `Sim::frame` a 60 FPS) lejos de cualquier planeta y verifica la respuesta de la nave.
use super::modes::*;
use super::Sim;
use crate::math::{DVec3, Vec3};
use crate::platform::{map_keys, KeyState};
use crate::world::solar_system;

const FPS: f64 = 60.0;

/// Muestra de cada cuadro.
#[derive(Clone, Copy, Debug)]
struct Sample {
    t: f64,
    speed: f64,
    /// Velocidad proyectada sobre la dirección inicial de vuelo (negativa = marcha atrás / rebote).
    along: f64,
    w: Vec3,
    /// Componente X del morro: >0 = giró a la derecha.
    fwd_x: f32,
}

/// Espacio abierto: lejos de todos los planetas (sin gravedad, sin marco local).
fn open_space() -> Sim {
    Sim::new(solar_system(), DVec3::new(0.0, 400_000.0, 0.0))
}

/// Ejecuta un guion `[(segundos, teclas)]`. Devuelve las muestras por cuadro.
fn run(sim: &mut Sim, script: &[(f64, KeyState)], dir: DVec3) -> Vec<Sample> {
    let mut out = Vec::new();
    for (secs, keys) in script {
        let input = map_keys(keys);
        for _ in 0..(secs * FPS).round() as usize {
            sim.frame(1.0 / FPS, &input);
            let fwd = sim.ship.orientation.rotate(Vec3::new(0.0, 0.0, -1.0));
            out.push(Sample {
                t: sim.t,
                speed: sim.ship.velocity.length(),
                along: sim.ship.velocity.dot(dir),
                w: sim.ship.angular_velocity,
                fwd_x: fwd.x,
            });
        }
    }
    out
}

fn k() -> KeyState {
    KeyState::default()
}
const FWD: DVec3 = DVec3 { x: 0.0, y: 0.0, z: -1.0 };

#[test]
fn thrust_response_is_progressive() {
    let mut sim = open_space();
    let tr = run(&mut sim, &[(3.0, KeyState { w: true, ..k() })], FWD);
    // la velocidad no salta: cambio por cuadro acotado por la aceleración máxima
    let mut prev = 0.0;
    for s in &tr {
        assert!((s.speed - prev).abs() <= ACCEL_FWD / FPS * 1.01, "salto de velocidad en t={:.2}", s.t);
        prev = s.speed;
    }
    // arranque suave: a 0.25 s todavía es poca velocidad, a 1 s ya se siente el empuje, y crece monótonamente
    let at = |t: f64| tr[(t * FPS) as usize - 1].speed;
    println!("empuje: 0.25s={:.1} 1s={:.1} 2s={:.1} 3s={:.1}", at(0.25), at(1.0), at(2.0), at(3.0));
    assert!(at(0.25) < 40.0, "arranque brusco: {}", at(0.25));
    assert!(at(1.0) > 150.0 && at(1.0) < 400.0, "a 1 s: {}", at(1.0));
    assert!(tr.windows(2).all(|w| w[1].speed >= w[0].speed - 1e-9));
}

#[test]
fn held_arrow_reaches_max_turn_rate_in_under_a_second() {
    let mut sim = open_space();
    let tr = run(&mut sim, &[(1.0, KeyState { right: true, ..k() })], FWD);
    let last = tr.last().unwrap();
    println!("giro: w(0.25s)={:.2} w(1s)={:.2} rad/s", tr[14].w.y, last.w.y);
    assert!(last.w.y.abs() > 0.9 * RATE_YAW as f32, "guiñada {} de {}", last.w.y, RATE_YAW);
    assert!(last.w.y.abs() <= RATE_YAW as f32 + 1e-4);
    assert!(last.fwd_x > 0.0, "flecha derecha debe girar a la derecha");
    // y a 0.25 s todavía no está al máximo (progresivo)
    assert!(tr[14].w.y.abs() < 0.8 * RATE_YAW as f32);
    // pitch: flecha arriba = morro arriba
    let mut sim = open_space();
    run(&mut sim, &[(0.5, KeyState { up: true, ..k() })], FWD);
    assert!(sim.ship.orientation.rotate(Vec3::new(0.0, 0.0, -1.0)).y > 0.0);
}

#[test]
fn releasing_turn_decays_without_overshoot() {
    let mut sim = open_space();
    let tr = run(&mut sim, &[(1.0, KeyState { left: true, ..k() }), (1.5, k())], FWD);
    let after: Vec<_> = tr.iter().skip(60).collect();
    // girando a la izquierda w.y > 0; al soltar no debe cambiar de signo
    assert!(after.iter().all(|s| s.w.y >= -1e-4), "la guiñada cambió de signo al soltar");
    assert!(after.windows(2).all(|w| w[1].w.y.abs() <= w[0].w.y.abs() + 1e-6));
    assert!(after.last().unwrap().w.y.abs() < 0.02, "no se detiene el giro");
}

#[test]
fn release_thrust_with_assist_brakes_without_oscillating() {
    let mut sim = open_space();
    let tr = run(&mut sim, &[(3.0, KeyState { w: true, ..k() }), (6.0, k())], FWD);
    let rel = 180; // cuadro de soltar
    let v_rel = tr[rel - 1].speed;
    assert!(v_rel > 300.0);
    // sin oscilación: tras el pico (la aceleración suavizada se apaga) la velocidad solo baja y nunca invierte
    let peak = tr.iter().enumerate().skip(rel).max_by(|a, b| a.1.speed.partial_cmp(&b.1.speed).unwrap()).unwrap().0;
    assert!(peak < rel + 30, "sigue acelerando mucho tras soltar");
    assert!(tr[peak..].windows(2).all(|w| w[1].speed <= w[0].speed + 1e-9), "oscila al frenar");
    assert!(tr[rel..].iter().all(|s| s.along >= -1e-6), "marcha atrás");
    let t_to_10pct = tr.iter().skip(rel).position(|s| s.speed < 0.1 * v_rel).expect("no llega al 10%") as f64 / FPS;
    println!("soltar empuje: v={v_rel:.0} -> 10% en {t_to_10pct:.2} s");
    assert!(t_to_10pct < 3.2, "asistencia demasiado flotante: {t_to_10pct}");
    // sin asistencia es newtoniano: mantiene la velocidad
    let mut sim = open_space();
    sim.ship.flight_assist = false;
    let tr = run(&mut sim, &[(3.0, KeyState { w: true, ..k() }), (4.0, k())], FWD);
    assert!(tr.last().unwrap().speed > 0.95 * tr[200].speed);
}

#[test]
fn boost_raises_speed_cap() {
    let mut a = open_space();
    let normal = run(&mut a, &[(10.0, KeyState { w: true, ..k() })], FWD);
    let mut b = open_space();
    let boosted = run(&mut b, &[(10.0, KeyState { w: true, shift: true, ..k() })], FWD);
    let (vn, vb) = (normal.last().unwrap().speed, boosted.last().unwrap().speed);
    println!("tope: normal={vn:.0} boost={vb:.0}");
    assert!(vn > 0.95 * CRUISE_SPEED && vn < 1.03 * CRUISE_SPEED, "tope normal {vn}");
    assert!(vb > 1.5 * CRUISE_SPEED && vb < 1.03 * BOOST_SPEED, "tope con boost {vb}");
    // el boost también acelera más fuerte al inicio
    assert!(boosted[60].speed > normal[60].speed * 1.2);
}

#[test]
fn brake_stops_the_ship() {
    let mut sim = open_space();
    let tr = run(&mut sim, &[(3.0, KeyState { w: true, ..k() }), (4.0, KeyState { ctrl: true, ..k() })], FWD);
    let v0 = tr[179].speed;
    let t_stop = tr.iter().skip(180).position(|s| s.speed < 5.0).expect("el freno no detiene") as f64 / FPS;
    println!("freno: {v0:.0} m/s -> <5 m/s en {t_stop:.2} s");
    assert!(t_stop < 3.5);
    assert!(tr[180..].iter().all(|s| s.along >= -1e-6));
}

/// Guion completo de 10 s: giro, empuje, boost, soltar y frenar. Todo finito y coherente.
#[test]
fn scripted_ten_second_flight() {
    let mut sim = open_space();
    let script = [
        (1.0, KeyState { right: true, ..k() }),
        (2.0, KeyState { w: true, ..k() }),
        (2.0, KeyState { w: true, shift: true, ..k() }),
        (2.0, k()),
        (3.0, KeyState { ctrl: true, ..k() }),
    ];
    let tr = run(&mut sim, &script, FWD);
    assert_eq!(tr.len(), 600);
    assert!(tr.iter().all(|s| s.speed.is_finite() && s.w.length().is_finite()));
    let max = tr.iter().map(|s| s.speed).fold(0.0, f64::max);
    assert!(max > 800.0 && max <= BOOST_SPEED * 1.03);
    assert!(tr.last().unwrap().speed < 10.0, "no termina frenada: {}", tr.last().unwrap().speed);
    assert!(tr.last().unwrap().w.length() < 1e-3);
    assert!(!sim.autopilot.active);
}
