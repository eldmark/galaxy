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
/// Límite numérico de seguridad de la velocidad en CRUCERO (m/s). Ya NO hay tope de jugabilidad: mantener el empuje
/// sigue subiendo la velocidad (la aceleración es finita y suavizada). Solo evita que los f32/f64 se desborden.
pub const SPEED_LIMIT: f64 = 200_000.0;
/// Boost sostenido estilo "warp": el multiplicador de aceleración sube de `BOOST_ACCEL` a `BOOST_ACCEL_MAX` con una
/// constante de tiempo `BOOST_RAMP_TAU` (s) según el tiempo que lleva el boost pulsado (`Ship::boost_charge`), y al
/// soltarlo la carga decae con `BOOST_DECAY_TAU`. Solo en CRUCERO y con el jugador (el autopiloto usa BOOST_ACCEL fijo).
pub const BOOST_ACCEL_MAX: f64 = 8.0;
pub const BOOST_RAMP_TAU: f64 = 6.0;
pub const BOOST_DECAY_TAU: f64 = 1.0;
/// Desaceleración máxima (m/s²) con que se recorta el exceso de velocidad sobre el tope POR ALTITUD de ORBITA y
/// DESCENSO (v_max = k·altitud). Al entrar a alta velocidad la nave frena de forma gradual (exceso/1.5 s, acotado a
/// este valor: ~400 g de "amortiguadores inerciales"); si aun así llega al suelo a más de IMPACT_DEATH_SPEED, se destruye.
pub const LOCAL_DECEL_MAX: f64 = 4000.0;
/// Desaceleración con que se recorta el exceso sobre SPEED_LIMIT en crucero.
pub const CAP_DECEL: f64 = 20_000.0;
/// v_max = k·altitud en ORBITA y DESCENSO (1/s), con suelo mínimo.
pub const ORBIT_SPEED_K: f64 = 1.0;
pub const DESCENT_SPEED_K: f64 = 0.5;
pub const MIN_SPEED_NEAR_GROUND: f64 = 3.0;
/// Amortiguamiento de la asistencia de vuelo y del freno (1/s).
/// Cambio (nav2): 0.7 -> 1.0. Con 0.7 la nave tardaba 3.5 s en bajar al 10 % de la velocidad al soltar el
/// empuje (se sentía "flotante" a ~1 km/s); con 1.0 baja en ~2.5 s y sigue siendo exponencial, sin oscilar.
pub const ASSIST_RATE: f64 = 1.0;
pub const BRAKE_RATE: f64 = 2.0;
/// Coeficientes de aterrizaje suave.
pub const LAND_MAX_VSPEED: f64 = 6.0;
pub const LAND_MAX_HSPEED: f64 = 12.0;
pub const LAND_MAX_TILT_DEG: f64 = 25.0;
/// Restitución del rebote en colisión.
pub const RESTITUTION: f64 = 0.2;

// ---- Peligros: sol, impacto y aterrizaje asistido ----
/// Distancia al centro del sol (en radios) por debajo de la cual la nave se destruye (altitud 0.5R = 4 km).
pub const SUN_KILL_R: f64 = 1.5;
/// Borde exterior de la zona de calor (en radios): el medidor de temperatura empieza a subir aquí.
pub const SUN_WARN_R: f64 = 2.3;
/// El casco solo sufre con calor > HEAT_DAMAGE_START (zona interior); la tasa máxima es HEAT_DAMAGE_RATE (casco/s).
pub const HEAT_DAMAGE_START: f64 = 0.55;
pub const HEAT_DAMAGE_RATE: f64 = 0.25;
/// Constantes de tiempo del medidor de calor al subir y al bajar (s).
pub const HEAT_RISE_TAU: f64 = 1.2;
pub const HEAT_FALL_TAU: f64 = 3.0;
/// Velocidad normal de impacto (m/s): por encima se destruye; entre BOUNCE_MIN y esta hay rebote con daño leve.
pub const IMPACT_DEATH_SPEED: f64 = 120.0;
pub const BOUNCE_MIN_SPEED: f64 = 12.0;
/// Segundos hasta la reaparición automática tras destruirse.
pub const RESPAWN_DELAY: f64 = 4.0;
/// Aterrizaje asistido: altitud de activación, velocidad máxima hacia el suelo y horizontal (m/s) y bloqueo tras cancelar (s).
pub const ASSIST_LAND_ALT: f64 = 250.0;
pub const ASSIST_LAND_VDOWN: f64 = 60.0;
pub const ASSIST_LAND_VHORIZ: f64 = 80.0;
pub const ASSIST_BLOCK_S: f64 = 3.0;
