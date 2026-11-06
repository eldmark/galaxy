//! Nave: física a paso fijo, gravedad, colisión y modos de navegación. Dueño: agente `nav`.
use crate::math::{DVec3, Quat, Vec3};
use crate::world::Body;

pub mod autopilot;
pub mod camera;
pub mod hazard;
#[cfg(test)]
mod flight_tests;
#[cfg(test)]
mod feel;
pub mod modes;
pub mod physics;
pub mod sim;
pub mod stepper;

pub use autopilot::Autopilot;
pub use camera::CameraRig;
pub use modes::NavMode;
pub use hazard::{DeathCause, ShipStatus, SimEvent};
pub use sim::Sim;
pub use stepper::FixedStepper;

/// Entrada normalizada del jugador (la rellena `platform`, la consume la física).
/// Ejes de la nave: +X derecha, +Y arriba, -Z adelante.
/// Giro: `rotate.x` > 0 = morro arriba, `rotate.y` > 0 = guiñada a la derecha, `rotate.z` > 0 = alabeo a la derecha.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Empuje deseado por eje en [-1, 1] (x lateral, y vertical, z: +1 = adelante).
    pub thrust: Vec3,
    /// Giro deseado en [-1, 1]: x = cabeceo, y = guiñada, z = alabeo.
    pub rotate: Vec3,
    pub boost: bool,
    pub brake: bool,
}

impl Input {
    /// ¿El jugador manda algo (empuje, giro o freno)? Se usa para interrumpir el autopiloto.
    pub fn is_active(&self) -> bool {
        self.thrust.length() > 0.05 || self.rotate.length() > 0.05 || self.brake
    }
}

#[derive(Clone, Debug)]
pub struct Ship {
    /// Posición en el marco del sistema (m, f64).
    pub position: DVec3,
    /// Velocidad en el marco del sistema (m/s).
    pub velocity: DVec3,
    pub orientation: Quat,
    /// Velocidad angular en ejes de la nave (rad/s).
    pub angular_velocity: Vec3,

    // ---- Campos de navegación (todos con valor por defecto en `Ship::new`) ----
    pub mode: NavMode,
    /// Cuerpo cuyo marco se usa en Orbit/Descent/Landed.
    pub frame: Option<usize>,
    /// Aceleración de empuje suavizada (marco del sistema, m/s²).
    pub accel: DVec3,
    /// Asistencia de vuelo (amortigua la velocidad respecto del cuerpo más cercano sin empuje). Tecla X.
    pub flight_assist: bool,
    /// Gravedad activa. Tecla G.
    pub gravity_enabled: bool,
    /// Fracción de gravedad cancelada en Orbit/Descent cuando la asistencia está activa (0..1).
    pub gravity_relief: f64,
    /// En Orbit, mantener la altitud al soltar el control vertical.
    pub hold_altitude: bool,
    /// El autopiloto la activa: el empuje se interpreta en ejes de la nave (sin remapeo local ni asistencia).
    pub raw_controls: bool,
    /// Tren de aterrizaje 0..1 (para animar la malla).
    pub gear: f32,
    /// Cuerpo con el que se hizo contacto en el último paso.
    pub contact: Option<usize>,
    /// Velocidad normal de impacto del último contacto (m/s).
    pub contact_speed: f64,
    /// Segundos desde el último cambio de modo (para mezclas de ~1.5 s).
    pub mode_time: f64,
    /// Aceleración gravitatoria efectiva del último paso (HUD).
    pub last_gravity: DVec3,
    /// Altitud sobre la superficie del cuerpo de referencia en el último paso.
    pub altitude: f64,
    /// Posición relativa al cuerpo en coordenadas fijas a él (solo Landed).
    pub landed_local: DVec3,
    pub alt_target: Option<f64>,
    pub takeoff_cooldown: f64,
    /// Segundos de boost sostenido (sube la aceleración estilo warp; decae al soltar). Solo la física la modifica.
    pub boost_charge: f64,
}

impl Ship {
    pub fn new(position: DVec3) -> Self {
        Self {
            position,
            velocity: DVec3::ZERO,
            orientation: Quat::IDENTITY,
            angular_velocity: Vec3::ZERO,
            mode: NavMode::Cruise,
            frame: None,
            accel: DVec3::ZERO,
            flight_assist: true,
            gravity_enabled: true,
            gravity_relief: 0.5,
            hold_altitude: true,
            raw_controls: false,
            gear: 0.0,
            contact: None,
            contact_speed: 0.0,
            mode_time: 100.0,
            last_gravity: DVec3::ZERO,
            altitude: f64::MAX,
            landed_local: DVec3::ZERO,
            alt_target: None,
            takeoff_cooldown: 0.0,
            boost_charge: 0.0,
        }
    }

    /// Un paso fijo de simulación (`dt` constante, p. ej. 1/120 s). `t` = tiempo de simulación actual.
    pub fn step(&mut self, dt: f64, t: f64, input: &Input, bodies: &[Body]) {
        physics::step(self, dt, t, input, bodies);
    }
}
