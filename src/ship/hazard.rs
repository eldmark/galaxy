//! Peligros de la nave: calor del sol, impacto y estado vivo/muerto. Funciones puras + tipos que usa `Sim`.
use super::modes::*;
use crate::math::DVec3;
use crate::world::smoothstep01;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    Heat,
    Impact,
    BlackHole,
    Police,
}

impl DeathCause {
    /// Texto del mensaje de destrucción.
    pub fn message(self) -> &'static str {
        match self {
            DeathCause::Heat => "NAVE DESTRUIDA: CALOR",
            DeathCause::Impact => "NAVE DESTRUIDA: IMPACTO",
            DeathCause::BlackHole => "NAVE DESTRUIDA: AGUJERO NEGRO",
            DeathCause::Police => "NAVE DESTRUIDA: POLICIA",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShipStatus {
    Alive,
    /// `since` = tiempo de simulación de la destrucción.
    Dead { cause: DeathCause, since: f64 },
}

/// Eventos que `Sim::frame` deja para el coordinador (efectos, sonido). Se vacían con `Sim::take_events`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SimEvent {
    Touchdown { body: usize },
    Bounce { body: usize, speed: f64 },
    Destroyed { cause: DeathCause },
    Respawned,
    AssistStarted,
    AssistCancelled,
}

/// Temperatura objetivo 0..1 a la distancia `d` del centro del sol (radio `sun_r`): 0 fuera de SUN_WARN_R,
/// 1 en SUN_KILL_R, suave entre ambas.
pub fn heat_target(d: f64, sun_r: f64) -> f64 {
    smoothstep01((SUN_WARN_R - d / sun_r) / (SUN_WARN_R - SUN_KILL_R))
}

/// Daño de casco por segundo para una temperatura `heat` (0 hasta HEAT_DAMAGE_START).
pub fn heat_damage_rate(heat: f64) -> f64 {
    ((heat - HEAT_DAMAGE_START) / (1.0 - HEAT_DAMAGE_START)).clamp(0.0, 1.0) * HEAT_DAMAGE_RATE
}

/// ¿El segmento p0->p1 entra en la esfera (centro c, radio r)? Devuelve el parámetro de entrada 0..1.
pub fn segment_sphere_entry(p0: DVec3, p1: DVec3, c: DVec3, r: f64) -> Option<f64> {
    let d = p1 - p0;
    let f = p0 - c;
    let a = d.dot(d);
    let cc = f.dot(f) - r * r;
    if cc <= 0.0 {
        return Some(0.0);
    }
    if a < 1e-12 {
        return None;
    }
    let b = f.dot(d);
    let disc = b * b - a * cc;
    if disc < 0.0 {
        return None;
    }
    let t = (-b - disc.sqrt()) / a;
    (0.0..=1.0).contains(&t).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_sphere() {
        let c = DVec3::ZERO;
        let hit = segment_sphere_entry(DVec3::new(-100.0, 0.0, 0.0), DVec3::new(100.0, 0.0, 0.0), c, 10.0).unwrap();
        assert!((hit - 0.45).abs() < 1e-9);
        assert!(segment_sphere_entry(DVec3::new(-100.0, 20.0, 0.0), DVec3::new(100.0, 20.0, 0.0), c, 10.0).is_none());
        assert!(segment_sphere_entry(DVec3::new(-100.0, 0.0, 0.0), DVec3::new(-50.0, 0.0, 0.0), c, 10.0).is_none());
    }

    #[test]
    fn heat_profile() {
        assert_eq!(heat_target(2.4 * 8000.0, 8000.0), 0.0);
        assert!((heat_target(1.5 * 8000.0, 8000.0) - 1.0).abs() < 1e-12);
        assert!(heat_target(2.0 * 8000.0, 8000.0) < HEAT_DAMAGE_START);
        assert_eq!(heat_damage_rate(0.3), 0.0);
        assert!(heat_damage_rate(1.0) > 0.0);
    }
}
