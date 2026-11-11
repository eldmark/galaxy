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
    /// Parámetro gravitacional GM (m^3/s^2), valores de jugabilidad, no físicos (el sol también atrae: g_superficie ~30 m/s²).
    pub gm: f64,
    pub landable: bool,
}

pub const TAU: f64 = std::f64::consts::TAU;

// ---- Utilidades vectoriales en f64 (math.rs no se toca) ----
pub fn cross(a: DVec3, b: DVec3) -> DVec3 {
    DVec3::new(a.y * b.z - a.z * b.y, a.z * b.x - a.x * b.z, a.x * b.y - a.y * b.x)
}
/// Rota `v` alrededor de +Y el ángulo `a` (misma convención que `Mat4::rotation_y`).
pub fn rot_y(v: DVec3, a: f64) -> DVec3 {
    let (s, c) = a.sin_cos();
    DVec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}
pub fn smoothstep01(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

impl Body {
    /// Posición en el marco del sistema (sol en el origen) en el instante `t` (segundos de simulación).
    /// Órbita circular inclinada: función analítica y periódica SOLO de `t` (no acumula estado).
    pub fn position(&self, t: f64) -> DVec3 {
        if self.orbit_radius <= 0.0 || self.orbit_period <= 0.0 {
            return DVec3::ZERO;
        }
        let a = self.phase + TAU * (t / self.orbit_period).fract();
        let (s, c) = a.sin_cos();
        let (si, ci) = self.inclination.sin_cos();
        DVec3::new(self.orbit_radius * c, self.orbit_radius * s * si, self.orbit_radius * s * ci)
    }
    /// Velocidad orbital (derivada analítica de `position`), m/s.
    pub fn velocity(&self, t: f64) -> DVec3 {
        if self.orbit_radius <= 0.0 || self.orbit_period <= 0.0 {
            return DVec3::ZERO;
        }
        let w = TAU / self.orbit_period;
        let a = self.phase + TAU * (t / self.orbit_period).fract();
        let (s, c) = a.sin_cos();
        let (si, ci) = self.inclination.sin_cos();
        let r = self.orbit_radius * w;
        DVec3::new(-r * s, r * c * si, r * c * ci)
    }
    /// Ángulo de rotación propia (rad) en el instante `t`, alrededor de +Y.
    pub fn rotation_angle(&self, t: f64) -> f64 {
        if self.rotation_period <= 0.0 {
            return 0.0;
        }
        TAU * (t / self.rotation_period).fract()
    }
    /// Velocidad angular de giro propio (rad/s).
    pub fn spin_rate(&self) -> f64 {
        if self.rotation_period <= 0.0 { 0.0 } else { TAU / self.rotation_period }
    }
    /// Esfera de influencia gravitatoria (m). Fuera de ella no atrae.
    /// Planetas: 8R. El sol: 3R (la órbita más interna, Fornax, queda en 2.75R pero dentro de su marco local solo
    /// cuenta la gravedad del planeta; ver `ship::physics::effective_gravity`).
    pub fn influence_radius(&self) -> f64 {
        if self.kind == BodyKind::Sun { 3.0 * self.radius } else { 8.0 * self.radius }
    }
    /// Fracción de la esfera de influencia donde empieza la atenuación (planetas 0.75; sol 0.9 para que 2.5R tenga
    /// gravedad casi completa y se pueda orbitar).
    pub fn gravity_fade_start(&self) -> f64 {
        if self.kind == BodyKind::Sun { 0.9 } else { 0.75 }
    }
    /// Velocidad circular de referencia a distancia `r` del centro: v = sqrt(GM·r/(r²+ε²)) (la que mantiene una órbita
    /// circular con la gravedad del juego). 0 si no hay gravedad.
    pub fn circular_speed(&self, r: f64) -> f64 {
        if self.gm <= 0.0 || r <= 0.0 {
            return 0.0;
        }
        let eps = self.gravity_epsilon();
        let fade_start = self.gravity_fade_start();
        let fade = 1.0 - smoothstep01((r / self.influence_radius() - fade_start) / (1.0 - fade_start));
        (self.gm * r * r / (r * r + eps * eps) * fade / r).max(0.0).sqrt()
    }
    /// Suavizado ε de la gravedad (m): a = GM / (r² + ε²).
    pub fn gravity_epsilon(&self) -> f64 {
        0.05 * self.radius
    }
    /// Aceleración gravitatoria (vector) que este cuerpo ejerce en `pos` cuando su centro está en `center`.
    /// Cae suavemente a 0 en el último 25 % de la esfera de influencia (sin saltos).
    pub fn gravity_at(&self, center: DVec3, pos: DVec3) -> DVec3 {
        if self.gm <= 0.0 {
            return DVec3::ZERO;
        }
        let d = center - pos;
        let r = d.length();
        let infl = self.influence_radius();
        if r >= infl || r < 1e-9 {
            return DVec3::ZERO;
        }
        let eps = self.gravity_epsilon();
        let fs = self.gravity_fade_start();
        let fade = 1.0 - smoothstep01((r / infl - fs) / (1.0 - fs));
        d * (self.gm / (r * r + eps * eps) * fade / r)
    }
    /// Vector del marco del mundo a partir de coordenadas fijas al cuerpo (gira con él).
    pub fn fixed_to_world(&self, t: f64, v: DVec3) -> DVec3 {
        rot_y(v, self.rotation_angle(t))
    }
    pub fn world_to_fixed(&self, t: f64, v: DVec3) -> DVec3 {
        rot_y(v, -self.rotation_angle(t))
    }
    /// Velocidad del suelo en el punto relativo `rel` (respecto del centro) por la rotación propia.
    pub fn spin_velocity(&self, rel: DVec3) -> DVec3 {
        cross(DVec3::new(0.0, self.spin_rate(), 0.0), rel)
    }
}

/// Altura de la superficie (m desde el centro) en la dirección `dir` (unitaria) expresada en el
/// marco FIJO al cuerpo (el de `relief::height` y el `p_local` de los shaders):
/// `radius * (1 + relief::height(kind, dir))`.
///
/// Decisión: esta función no recibe el tiempo, y la rotación propia del cuerpo cambia qué punto del
/// terreno queda bajo una dirección del mundo. Por eso hay dos versiones: esta (marco fijo) y
/// [`surface_height_at`] (dirección en marco del mundo + instante), que es la que usan física y piloto.
pub fn surface_height(body: &Body, dir: DVec3) -> f64 {
    let d = dir.to_f32();
    body.radius * (1.0 + crate::relief::height(body.kind, d) as f64)
}

/// Altura de la superficie en la dirección `dir_world` (unitaria, marco del mundo) en el instante `t`.
pub fn surface_height_at(body: &Body, t: f64, dir_world: DVec3) -> f64 {
    surface_height(body, body.world_to_fixed(t, dir_world))
}

/// Altitud real de `pos` sobre el terreno del cuerpo (usa el relieve), m.
pub fn terrain_altitude(body: &Body, t: f64, pos: DVec3) -> f64 {
    let rel = pos - body.position(t);
    let d = rel.length();
    if d < 1e-9 {
        return -body.radius;
    }
    d - surface_height_at(body, t, rel * (1.0 / d))
}

/// Altitud de `ship_pos` sobre la esfera (radio) de un cuerpo en `body_pos`.
pub fn altitude(body_pos: DVec3, radius: f64, ship_pos: DVec3) -> f64 {
    (ship_pos - body_pos).length() - radius
}

/// Índice del cuerpo con menor altitud (distancia a la superficie) en el instante `t`.
pub fn nearest_body(bodies: &[Body], pos: DVec3, t: f64) -> usize {
    let mut best = 0;
    let mut best_alt = f64::MAX;
    for (i, b) in bodies.iter().enumerate() {
        let a = altitude(b.position(t), b.radius, pos);
        if a < best_alt {
            best_alt = a;
            best = i;
        }
    }
    best
}

/// Matriz de modelo (traslación relativa al ojo en f64 -> f32, rotación propia, escala de radio) para dibujar un cuerpo.
/// `eye` es la posición del mundo del origen de la cámara (origen flotante).
pub fn body_model_matrix(body: &Body, t: f64, eye: DVec3) -> crate::math::Mat4 {
    use crate::math::{Mat4, Vec3};
    let rel = (body.position(t) - eye).to_f32();
    Mat4::translation(rel)
        * Mat4::rotation_y(body.rotation_angle(t) as f32)
        * Mat4::scale(Vec3::new(body.radius as f32, body.radius as f32, body.radius as f32))
}

/// Tabla de cuerpos: sol primero. Valores iniciales, se ajustan jugando.
pub fn solar_system() -> Vec<Body> {
    use BodyKind::*;
    let b = |kind, name, radius, orbit_radius, orbit_period, rotation_period, inclination, phase, gm, landable| Body {
        kind, name, radius, orbit_radius, orbit_period, rotation_period, inclination, phase, gm, landable,
    };
    // Jugabilidad: gravedad superficial ~30 m/s^2 (gm = 30 R^2); planetas se mueven ~280 m/s.
    let g = |r: f64| 30.0 * r * r;
    vec![
        b(Sun, "Ignea", 8000.0, 0.0, 0.0, 240.0, 0.0, 0.0, g(8000.0), false),
        b(Fornax, "Fornax", 3000.0, 22000.0, 500.0, 70.0, 0.05, 0.5, g(3000.0), true),
        b(Verdalia, "Verdalia", 3500.0, 35000.0, 800.0, 90.0, 0.08, 2.0, g(3500.0), true),
        b(Mutante, "Mutante", 2000.0, 55000.0, 1200.0, 60.0, 0.10, 4.0, g(2000.0), true),
        // La capa superior de nubes funciona como plataforma flotante para el modo pirata.
        b(Siroco, "Siroco", 6000.0, 80000.0, 1800.0, 45.0, 0.04, 1.0, g(6000.0), true),
        b(Nix, "Nix", 2500.0, 105000.0, 2400.0, 110.0, 0.12, 5.2, g(2500.0), true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_is_periodic_and_stateless() {
        for b in solar_system().iter().skip(1) {
            let p0 = b.position(37.0);
            let p1 = b.position(37.0 + b.orbit_period);
            assert!((p0 - p1).length() < 1e-5, "{}", b.name);
            assert!((b.position(37.0) - p0).length() == 0.0);
            assert!((p0.length() - b.orbit_radius).abs() < 1e-6);
        }
    }

    #[test]
    fn velocity_matches_finite_difference() {
        let sys = solar_system();
        let b = &sys[2];
        let h = 1e-3;
        let fd = (b.position(100.0 + h) - b.position(100.0 - h)) * (1.0 / (2.0 * h));
        assert!((fd - b.velocity(100.0)).length() < 1e-3);
    }

    #[test]
    fn rot_y_matches_matrix() {
        let v = rot_y(DVec3::new(1.0, 2.0, 3.0), 0.7);
        let m = crate::math::Mat4::rotation_y(0.7).transform_dir(crate::math::Vec3::new(1.0, 2.0, 3.0));
        assert!((v.to_f32() - m).length() < 1e-5);
    }
}

#[cfg(test)]
mod gravity_tests {
    use super::*;

    #[test]
    fn sun_has_gravity_and_bounded_influence() {
        let sys = solar_system();
        let sun = &sys[0];
        assert!(sun.gm > 0.0);
        let g_surf = sun.gravity_at(DVec3::ZERO, DVec3::new(sun.radius, 0.0, 0.0)).length();
        assert!((g_surf - 30.0).abs() < 3.0, "g superficie {g_surf}");
        assert_eq!(sun.gravity_at(DVec3::ZERO, DVec3::new(3.0 * sun.radius, 0.0, 0.0)).length(), 0.0);
        // a 2.5R todavía hay gravedad apreciable y velocidad circular razonable
        let v = sun.circular_speed(2.5 * sun.radius);
        assert!(v > 200.0 && v < 450.0, "vc sol 2.5R = {v}");
    }

    #[test]
    fn circular_speed_matches_gravity() {
        for b in solar_system().iter() {
            let r = 1.5 * b.radius;
            let a = b.gravity_at(DVec3::ZERO, DVec3::new(r, 0.0, 0.0)).length();
            let v = b.circular_speed(r);
            assert!((v * v / r - a).abs() < 1e-6 * a.max(1.0), "{}", b.name);
        }
    }
}
