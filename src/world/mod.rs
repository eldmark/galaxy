//! Cuerpos del sistema solar y movimiento orbital. Dueño: agente `nav` (world + ship).
use crate::math::DVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Sun,      // Ígnea
    Verdalia, // rocoso con océanos, nubes
    Siroco,   // gigante gaseoso con anillo
    Mutante,  // pixelado
    Nix,      // glacial
    Fornax,   // volcánico
}

#[derive(Clone, Debug)]
pub struct Body {
    pub kind: BodyKind,
    pub name: &'static str,
    /// Metros.
    pub radius: f64,
    /// Radio de la órbita circular alrededor del sol (m). 0 para el sol.
    pub orbit_radius: f64,
    /// Segundos reales de una vuelta orbital. 0 para el sol.
    pub orbit_period: f64,
    /// Segundos de una rotación completa sobre su eje.
    pub rotation_period: f64,
    /// Inclinación de la órbita (rad).
    pub inclination: f64,
    /// Fase inicial (rad).
    pub phase: f64,
    /// Parámetro gravitacional GM (m^3/s^2), valores de jugabilidad, no físicos.
    pub gm: f64,
    pub landable: bool,
}

impl Body {
    /// Posición en el marco del sistema (sol en el origen) en el instante `t` (segundos de simulación).
    pub fn position(&self, _t: f64) -> DVec3 {
        todo!("nav")
    }
    /// Ángulo de rotación propia (rad) en el instante `t`.
    pub fn rotation_angle(&self, _t: f64) -> f64 {
        todo!("nav")
    }
}

/// Tabla de cuerpos: sol primero. Valores iniciales, se ajustan jugando.
pub fn solar_system() -> Vec<Body> {
    use BodyKind::*;
    let b = |kind, name, radius, orbit_radius, orbit_period, rotation_period, inclination, phase, gm, landable| Body {
        kind, name, radius, orbit_radius, orbit_period, rotation_period, inclination, phase, gm, landable,
    };
    vec![
        b(Sun, "Ignea", 8000.0, 0.0, 0.0, 240.0, 0.0, 0.0, 0.0, false),
        b(Fornax, "Fornax", 3000.0, 22000.0, 150.0, 70.0, 0.05, 0.5, 4.0e9, true),
        b(Verdalia, "Verdalia", 3500.0, 35000.0, 220.0, 90.0, 0.08, 2.0, 6.0e9, true),
        b(Mutante, "Mutante", 2000.0, 55000.0, 300.0, 60.0, 0.10, 4.0, 2.0e9, true),
        b(Siroco, "Siroco", 6000.0, 80000.0, 420.0, 45.0, 0.04, 1.0, 1.4e10, false),
        b(Nix, "Nix", 2500.0, 105000.0, 540.0, 110.0, 0.12, 5.2, 3.0e9, true),
    ]
}
