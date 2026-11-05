//! Modos de navegación y constantes de jugabilidad.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavMode {
    /// Marco del sistema; vuelo libre con inercia.
    Cruise,
    /// Marco del planeta (órbita local): la nave hereda el movimiento orbital del cuerpo.
    Orbit,
    /// Cerca del suelo: marco superficial mezclado, límite de velocidad ∝ altitud, horizonte estabilizado.
    Descent,
    /// Posada: fija al marco superficial (gira con el planeta).
    Landed,
}

impl NavMode {
    pub fn name(self) -> &'static str {
        match self {
            NavMode::Cruise => "CRUCERO",
            NavMode::Orbit => "ORBITA",
            NavMode::Descent => "DESCENSO",
            NavMode::Landed => "POSADA",
        }
    }
}

// ---- Constantes de jugabilidad (m, s) ----
pub const SHIP_RADIUS: f64 = 6.0;
/// Entrada/salida de ORBITA en radios del planeta (histéresis: entrada < salida).
pub const ORBIT_ENTER: f64 = 3.0;
pub const ORBIT_EXIT: f64 = 3.5;
/// Entrada/salida de DESCENSO.
pub const DESCENT_ENTER: f64 = 0.5;
pub const DESCENT_EXIT: f64 = 0.7;
/// Aceleración máxima por eje (m/s²) y multiplicador de boost.
pub const ACCEL_FWD: f64 = 400.0;
pub const ACCEL_LAT: f64 = 240.0;
pub const ACCEL_VERT: f64 = 240.0;
pub const BOOST_ACCEL: f64 = 1.5;
/// Constante de tiempo de la aceleración (suavizado exponencial) y del giro.
pub const TAU_ACCEL: f64 = 0.35;
pub const TAU_ANGULAR: f64 = 0.25;
/// Velocidades angulares máximas (rad/s): cabeceo, guiñada, alabeo.
pub const RATE_PITCH: f64 = 1.2;
pub const RATE_YAW: f64 = 1.0;
pub const RATE_ROLL: f64 = 1.6;
/// Velocidad máxima de crucero y con boost (m/s).
pub const CRUISE_SPEED: f64 = 1000.0;
pub const BOOST_SPEED: f64 = 2000.0;
/// v_max = k·altitud en ORBITA y DESCENSO (1/s), con suelo mínimo.
pub const ORBIT_SPEED_K: f64 = 1.0;
pub const DESCENT_SPEED_K: f64 = 0.5;
pub const MIN_SPEED_NEAR_GROUND: f64 = 3.0;
/// Amortiguamiento de la asistencia de vuelo y del freno (1/s).
pub const ASSIST_RATE: f64 = 0.7;
pub const BRAKE_RATE: f64 = 2.0;
/// Coeficientes de aterrizaje suave.
pub const LAND_MAX_VSPEED: f64 = 6.0;
pub const LAND_MAX_HSPEED: f64 = 12.0;
pub const LAND_MAX_TILT_DEG: f64 = 25.0;
/// Restitución del rebote en colisión.
pub const RESTITUTION: f64 = 0.2;
