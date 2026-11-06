//! Atmósfera por cuerpo: halo desde el espacio, cielo degradado desde dentro y niebla de perspectiva.
//!
//! Todo sale de UNA integración por rayo (dispersión simple): para cada píxel se calcula la dirección en el
//! mundo, se intersecta analíticamente con la esfera exterior (R + altura) y la interior (R), se recorta el
//! segmento con la profundidad del z-buffer (`fb.depth` = 1/w, y con el rayo sin normalizar de z = -1 la
//! distancia paramétrica es exactamente `w`), y se acumula densidad exponencial con 3 muestras. El resultado
//! es una mezcla `bg * (1 - alpha) + color * alpha`, que no toca el z-buffer ni la silueta de los planetas.
//!
//! - Rayo que falla el planeta: halo (desde fuera) o cielo (desde dentro); las estrellas del fondo se
//!   atenúan con `alpha` y la luz del sol en cada muestra, así que desaparecen de día y vuelven de noche.
//! - Rayo que termina en geometría: niebla de perspectiva hacia el color del horizonte. Desde fuera se usa
//!   una bruma mucho más débil, salvo en el borde del disco (incidencia rasante) donde se empalma con el halo.
use crate::math::{Mat4, Vec3};
use crate::raster::Framebuffer;
use crate::world::{Body, BodyKind};
use rayon::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    /// Color del cielo en el cenit con sol alto.
    pub color_day: [f32; 3],
    /// Color hacia el horizonte (también el de la niebla).
    pub color_horizon: [f32; 3],
    /// Color cerca del terminador (atardecer).
    pub color_sunset: [f32; 3],
    /// Altura de la capa (m). Por encima no hay efecto alguno.
    pub height_m: f32,
    /// Opacidad vertical del cielo (1 = como Verdalia).
    pub density: f32,
    /// Extinción de la niebla a nivel del suelo (1/m).
    pub fog_per_m: f32,
}

/// Parámetros de cada cuerpo; `None` = sin atmósfera (el sol y... nada más; Mutante tiene una muy tenue).
pub fn for_kind(kind: BodyKind) -> Option<Atmosphere> {
    match kind {
        BodyKind::Sun => None,
        BodyKind::Verdalia => Some(Atmosphere {
            color_day: [0.30, 0.55, 0.95],
            color_horizon: [0.72, 0.84, 0.96],
            color_sunset: [1.00, 0.52, 0.26],
            height_m: 2500.0,
            density: 1.0,
            fog_per_m: 1.3e-4,
        }),
        BodyKind::Nix => Some(Atmosphere {
            color_day: [0.52, 0.72, 0.90],
            color_horizon: [0.82, 0.91, 0.97],
            color_sunset: [0.72, 0.80, 1.00],
            height_m: 900.0,
            density: 0.45,
            fog_per_m: 9.0e-5,
        }),
        BodyKind::Fornax => Some(Atmosphere {
            color_day: [0.50, 0.22, 0.16],
            color_horizon: [0.78, 0.40, 0.22],
            color_sunset: [1.00, 0.34, 0.10],
            height_m: 1400.0,
            density: 0.85,
            fog_per_m: 3.2e-4,
        }),
        BodyKind::Mutante => Some(Atmosphere {
            color_day: [0.55, 0.42, 0.62],
            color_horizon: [0.80, 0.66, 0.76],
            color_sunset: [1.00, 0.62, 0.50],
            height_m: 300.0,
            density: 0.12,
            fog_per_m: 3.0e-5,
        }),
        BodyKind::Siroco => Some(Atmosphere {
            color_day: [0.95, 0.66, 0.30],
            color_horizon: [1.00, 0.78, 0.45],
            color_sunset: [1.00, 0.55, 0.20],
            height_m: 1500.0,
            density: 0.5,
            fog_per_m: 0.0,
        }),
    }
}

// ------------------------------------------------------------------ funciones puras

/// Intersección rayo-esfera. `d` unitario, `c` centro relativo al origen del rayo. Devuelve (t_ent, t_sal)
/// con `t_ent` posiblemente negativo (origen dentro), o `None` si falla.
pub fn ray_sphere(d: Vec3, c: Vec3, r: f32) -> Option<(f32, f32)> {
    let b = d.dot(c);
    let disc = r * r - (c.length_sq() - b * b);
    if disc < 0.0 || !disc.is_finite() {
        return None;
    }
    let s = disc.sqrt();
    Some((b - s, b + s))
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Densidad relativa (1 al nivel del suelo, 0 a la altura de la capa), continua en el borde.
pub fn density_at(altitude: f32, height: f32) -> f32 {
    if altitude >= height || height <= 0.0 {
        return 0.0;
    }
    const K: f32 = 4.5;
    let e = (-K * altitude.max(0.0) / height).exp();
    let floor = (-K).exp();
    (e - floor) / (1.0 - floor)
}

/// Fracción de "cielo" (0 = espacio puro, 1 = cielo opaco) según la altitud del observador: monótona
/// decreciente; 0 en y por encima de `height`. Sirve para atenuar las estrellas al bajar.
pub fn sky_amount(altitude: f32, height: f32) -> f32 {
    if height <= 0.0 {
        return 0.0;
    }
    1.0 - smooth(0.0, height, altitude)
}

/// Luz solar en un punto de la atmósfera según el coseno entre la vertical local y el sol.
fn sun_light(mu: f32) -> f32 {
    smooth(-0.18, 0.32, mu)
}

/// Cuerpo con atmósfera preparado para el cuadro actual (todo relativo al ojo, en f32).
#[derive(Clone, Copy, Debug)]
pub struct AtmBody {
    pub atm: Atmosphere,
    /// Centro del planeta relativo al ojo (m).
    pub center: Vec3,
    pub radius: f32,
    /// Dirección unitaria hacia el sol.
    pub sun: Vec3,
    /// Altitud del ojo sobre la esfera (m).
    pub eye_alt: f32,
}

/// Resultado por píxel: `out = bg * (1 - alpha) + color * alpha`.
#[derive(Clone, Copy, Debug)]
pub struct Effect {
    pub color: [f32; 3],
    pub alpha: f32,
}

const SAMPLES: usize = 3;

impl AtmBody {
    pub fn new(atm: Atmosphere, body_pos: Vec3, radius: f32, sun: Vec3) -> Self {
        let center = body_pos;
        Self { atm, center, radius, sun, eye_alt: center.length() - radius }
    }

    fn outer(&self) -> f32 {
        self.radius + self.atm.height_m
    }
    pub fn inside(&self) -> bool {
        self.eye_alt < self.atm.height_m
    }

    /// Efecto sobre el píxel con dirección unitaria `d`. `geo` = distancia al fragmento con geometría
    /// (`f32::INFINITY` si es fondo). `None` si el rayo no cruza la atmósfera antes de la geometría.
    pub fn effect(&self, d: Vec3, geo: f32) -> Option<Effect> {
        let a = &self.atm;
        if a.height_m <= 0.0 || a.density <= 0.0 {
            return None;
        }
        let (t_in, t_out) = ray_sphere(d, self.center, self.outer())?;
        if t_out <= 0.0 {
            return None;
        }
        let t0 = t_in.max(0.0);
        let mut t1 = t_out;
        let mut ended = false;
        let mut sky_ground = false; // toca la esfera pero sin geometría dibujada: se trata como horizonte
        if let Some((pi, _)) = ray_sphere(d, self.center, self.radius) {
            if pi > 0.0 && pi < t1 {
                t1 = pi;
                ended = true;
                sky_ground = geo.is_infinite();
            }
        }
        if geo < t1 {
            if geo <= t0 {
                return None;
            }
            t1 = geo;
            ended = true;
        }
        let len = t1 - t0;
        if len <= 1e-3 {
            return None;
        }
        // Columna de aire y color medio ponderado por densidad y luz del sol.
        let hs = a.height_m;
        let ds = len / SAMPLES as f32;
        let (mut col_acc, mut od, mut lit_acc) = ([0.0f32; 3], 0.0f32, 0.0f32);
        for i in 0..SAMPLES {
            let s = t0 + (i as f32 + 0.5) * ds;
            let v = d * s - self.center;
            let h = v.length();
            let up = v * (1.0 / h.max(1e-3));
            let rho = density_at(h - self.radius, hs);
            let w = rho * ds;
            let mu = up.dot(self.sun);
            let lit = sun_light(mu);
            let hz = { let q = 1.0 - d.dot(up).abs(); q * q };
            let base = mix3(a.color_day, a.color_horizon, hz);
            let facing = 0.5 + 0.5 * d.dot(self.sun);
            let sunset = (-(mu / 0.22) * (mu / 0.22)).exp() * (0.3 + 0.7 * facing);
            let c = mix3(base, a.color_sunset, sunset);
            for k in 0..3 {
                col_acc[k] += c[k] * w;
            }
            od += w;
            lit_acc += lit * w;
        }
        if od <= 1e-6 {
            return None;
        }
        let mut col = [col_acc[0] / od, col_acc[1] / od, col_acc[2] / od];
        let lit = lit_acc / od;
        // Extinción por metro: cielo opaco en la vertical con density = 1 (od vertical ~ 2.4).
        let sky_sigma = 2.4 * a.density / (hs * 0.28);
        let sigma = if sky_ground {
            // sin geometría (chunks lejanos sin dibujar): opaco hacia el color del horizonte
            col = mix3(col, a.color_horizon, 0.8);
            sky_sigma * 3.0
        } else if ended {
            // normal en el punto final: la incidencia rasante enlaza con el halo
            let p = d * t1 - self.center;
            let n = p * (1.0 / p.length().max(1e-3));
            let mu = d.dot(n).abs();
            let limb = (-mu / 0.18).exp();
            let haze = sky_sigma * 0.10;
            let surface = haze + (sky_sigma - haze) * limb;
            let w_in = 1.0 - smooth(0.5 * hs, hs, self.eye_alt);
            let fog = a.fog_per_m;
            // empalme: con el ojo dentro manda la niebla; fuera, la bruma
            let col_fog = mix3(col, a.color_horizon, 0.75);
            col = mix3(col, col_fog, w_in);
            surface + (fog - surface) * w_in
        } else {
            sky_sigma
        };
        let alpha = (1.0 - (-sigma * od).exp()) * lit.max(if ended && !sky_ground && self.inside() { 0.06 } else { 0.0 });
        let alpha = if alpha.is_finite() { alpha.clamp(0.0, 0.97) } else { 0.0 };
        if alpha <= 0.0 {
            return None;
        }
        Some(Effect { color: col, alpha })
    }
}

/// Aplica la atmósfera a todo el cuadro. Debe llamarse después de `render_scene` y antes del HUD.
/// `eye` es la posición del ojo (mundo) y `view` la matriz de vista sin traslación.
pub fn apply(fb: &mut Framebuffer, view: Mat4, fov_y: f32, eye: crate::math::DVec3, bodies: &[Body], t: f64) {
    let mut list: Vec<AtmBody> = Vec::new();
    for b in bodies {
        let Some(atm) = for_kind(b.kind) else { continue };
        let pos = b.position(t);
        let rel = (pos - eye).to_f32();
        let sun = if pos.length() > 1.0 { (-pos).normalized().to_f32() } else { Vec3::Y };
        let ab = AtmBody::new(atm, rel, b.radius as f32, sun);
        // descarta cuerpos cuya atmósfera queda muy lejos del ojo (ni siquiera ocupa un píxel)
        if rel.length() - ab.outer() > 40.0 * ab.outer() {
            continue;
        }
        list.push(ab);
    }
    if list.is_empty() {
        return;
    }
    let (w, h) = (fb.width, fb.height);
    let tn = (fov_y * 0.5).tan();
    let aspect = w as f32 / h as f32;
    let inv = view.transpose();
    // Cono de cada cuerpo (coseno del ángulo máximo) para descartar rápido los píxeles que no lo tocan.
    let cones: Vec<(f32, Vec3)> = list
        .iter()
        .map(|b| {
            let dist = b.center.length();
            let r = b.outer();
            let cos = if dist > r { (1.0 - (r / dist) * (r / dist)).max(0.0).sqrt() } else { -2.0 };
            (cos - 0.002, b.center * (1.0 / dist.max(1e-3)))
        })
        .collect();
    let depth = &fb.depth;
    // Bloques de 2x2 píxeles: el efecto es suave, así que se evalúa una vez por bloque cuando los cuatro
    // son fondo o tienen profundidad parecida; si no (bordes de geometría) se evalúa por píxel.
    fb.color.par_chunks_mut(w * 8).zip(depth.par_chunks(w * 2)).enumerate().for_each(|(by, (rows, drows))| {
        let nrows = rows.len() / (w * 4);
        let mut eval = |rows: &mut [u8], px: usize, py: usize, d: Vec3, geo: f32, count_x: usize, count_y: usize| {
            let mut acc: Option<[f32; 3]> = None;
            // un mismo efecto se aplica a todos los píxeles del bloque, cada uno sobre su propio color
            let mut effects: [Option<Effect>; 4] = [None; 4];
            let mut n = 0;
            for (b, (cos, axis)) in list.iter().zip(&cones) {
                if d.dot(*axis) < *cos {
                    continue;
                }
                if let Some(e) = b.effect(d, geo) {
                    effects[n] = Some(e);
                    n += 1;
                }
            }
            let _ = &mut acc;
            if n == 0 {
                return;
            }
            for yy in 0..count_y {
                for xx in 0..count_x {
                    let o = (py + yy) * w * 4 + (px + xx) * 4;
                    let mut cur = [rows[o] as f32 / 255.0, rows[o + 1] as f32 / 255.0, rows[o + 2] as f32 / 255.0];
                    for e in effects[..n].iter().flatten() {
                        cur = [0, 1, 2].map(|k| cur[k] * (1.0 - e.alpha) + e.color[k] * e.alpha);
                    }
                    for k in 0..3 {
                        rows[o + k] = (cur[k].clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                    }
                }
            }
        };
        let ray_at = |fx: f32, fy: f32| {
            let ny = 1.0 - fy / h as f32 * 2.0;
            let nx = fx / w as f32 * 2.0 - 1.0;
            let ray = inv.transform_dir(Vec3::new(nx * aspect * tn, ny * tn, -1.0));
            let len = ray.length();
            (ray * (1.0 / len), len)
        };
        let mut x = 0;
        while x < w {
            let nx_ = (w - x).min(2);
            let y0 = by * 2;
            let ds = [drows[x], if nx_ > 1 { drows[x + 1] } else { drows[x] }, if nrows > 1 { drows[w + x] } else { drows[x] }, if nrows > 1 && nx_ > 1 { drows[w + x + 1] } else { drows[x] }];
            let (mn, mx) = ds.iter().fold((f32::MAX, 0.0f32), |(a, b), &v| (a.min(v), b.max(v)));
            let uniform = (mx == 0.0) || (mn > 0.0 && mx < mn * 1.12);
            if uniform {
                let (d, len) = ray_at(x as f32 + nx_ as f32 * 0.5, y0 as f32 + nrows as f32 * 0.5);
                let geo = if mx == 0.0 { f32::INFINITY } else { len / (ds.iter().sum::<f32>() * 0.25) };
                eval(rows, x, 0, d, geo, nx_, nrows);
            } else {
                for yy in 0..nrows {
                    for xx in 0..nx_ {
                        let (d, len) = ray_at((x + xx) as f32 + 0.5, (y0 + yy) as f32 + 0.5);
                        let dv = drows[yy * w + x + xx];
                        let geo = if dv > 0.0 { len / dv } else { f32::INFINITY };
                        eval(rows, x + xx, yy, d, geo, 1, 1);
                    }
                }
            }
            x += 2;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdalia(eye_alt: f32, sun: Vec3) -> AtmBody {
        // el ojo en +Y sobre un planeta de 3500 m; el centro queda a -(R + alt) en Y
        AtmBody::new(for_kind(BodyKind::Verdalia).unwrap(), Vec3::new(0.0, -(3500.0 + eye_alt), 0.0), 3500.0, sun)
    }

    #[test]
    fn ray_sphere_analytic() {
        let (a, b) = ray_sphere(Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.0, 0.0, -10.0), 2.0).unwrap();
        assert!((a - 8.0).abs() < 1e-5 && (b - 12.0).abs() < 1e-5);
        assert!(ray_sphere(Vec3::new(0.0, 0.0, -1.0), Vec3::new(5.0, 0.0, -10.0), 2.0).is_none());
        // origen dentro: entrada negativa
        let (a, b) = ray_sphere(Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, 3.0).unwrap();
        assert!((a + 3.0).abs() < 1e-5 && (b - 3.0).abs() < 1e-5);
        // tangente
        assert!(ray_sphere(Vec3::new(0.0, 0.0, -1.0), Vec3::new(2.0, 0.0, -10.0), 2.0).is_some());
    }

    #[test]
    fn density_and_sky_monotone() {
        let mut prev = 2.0;
        let mut prev_sky = 2.0;
        for i in 0..=100 {
            let alt = i as f32 * 30.0;
            let d = density_at(alt, 2500.0);
            let s = sky_amount(alt, 2500.0);
            assert!(d <= prev && s <= prev_sky && (0.0..=1.0).contains(&d) && (0.0..=1.0).contains(&s));
            prev = d;
            prev_sky = s;
        }
        assert_eq!(density_at(2500.0, 2500.0), 0.0);
        assert_eq!(sky_amount(2500.0, 2500.0), 0.0);
        assert_eq!(sky_amount(9000.0, 2500.0), 0.0);
        assert!((density_at(0.0, 2500.0) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn no_effect_outside_or_without_atmosphere() {
        let sun = Vec3::new(1.0, 0.3, 0.0).normalized();
        // ojo lejos (50 km) mirando a un lado: sin efecto
        let b = verdalia(50_000.0, sun);
        assert!(b.effect(Vec3::new(1.0, 0.0, 0.0), f32::INFINITY).is_none());
        assert!(b.effect(Vec3::new(0.0, 1.0, 0.0), f32::INFINITY).is_none());
        // hacia el planeta pero con geometría delante de la atmósfera: sin efecto
        assert!(b.effect(Vec3::new(0.0, -1.0, 0.0), 100.0).is_none());
        // cuerpo sin atmósfera
        assert!(for_kind(BodyKind::Sun).is_none());
        let mut a = for_kind(BodyKind::Nix).unwrap();
        a.density = 0.0;
        let b = AtmBody::new(a, Vec3::new(0.0, -2540.0, 0.0), 2500.0, sun);
        assert!(b.effect(Vec3::Y, f32::INFINITY).is_none());
    }

    #[test]
    fn effects_are_finite_and_bounded() {
        let suns = [Vec3::Y, -Vec3::Y, Vec3::X, Vec3::new(0.3, 0.05, 0.9).normalized()];
        for kind in [BodyKind::Verdalia, BodyKind::Nix, BodyKind::Fornax, BodyKind::Mutante, BodyKind::Siroco] {
            let atm = for_kind(kind).unwrap();
            for alt in [-1.0, 0.0, 40.0, 500.0, 1500.0, 2400.0, 6000.0, 20_000.0] {
                for &sun in &suns {
                    let b = AtmBody::new(atm, Vec3::new(0.0, -(3000.0 + alt), 0.0), 3000.0, sun);
                    for i in 0..60 {
                        let a = i as f32 / 60.0 * std::f32::consts::TAU;
                        let d = Vec3::new(a.cos(), a.sin() * 0.8 - 0.1, (a * 3.0).sin()).normalized();
                        for geo in [f32::INFINITY, 50.0, 5000.0] {
                            if let Some(e) = b.effect(d, geo) {
                                assert!(e.alpha.is_finite() && (0.0..=1.0).contains(&e.alpha), "{kind:?} {alt}");
                                assert!(e.color.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sky_alpha_decreases_with_altitude() {
        // mirando al cenit a pleno día, el cielo se vuelve más transparente al subir
        let mut prev = 2.0;
        for alt in [0.0, 300.0, 900.0, 1500.0, 2200.0, 2499.0] {
            let b = verdalia(alt, Vec3::Y);
            let a = b.effect(Vec3::Y, f32::INFINITY).map(|e| e.alpha).unwrap_or(0.0);
            assert!(a <= prev + 1e-6, "alt {alt}: {a} > {prev}");
            prev = a;
        }
        assert!(verdalia(0.0, Vec3::Y).effect(Vec3::Y, f32::INFINITY).unwrap().alpha > 0.7);
        assert!(verdalia(2499.9, Vec3::Y).effect(Vec3::Y, f32::INFINITY).is_none_or(|e| e.alpha < 0.01));
    }

    #[test]
    fn night_hides_sky_and_fog_grows_with_distance() {
        // noche: casi sin cielo (las estrellas se ven)
        let night = verdalia(10.0, -Vec3::Y);
        assert!(night.effect(Vec3::Y, f32::INFINITY).is_none_or(|e| e.alpha < 0.05));
        // niebla: más distancia, más alfa (rayo casi horizontal sobre el suelo)
        let day = verdalia(40.0, Vec3::Y);
        let d = Vec3::new(1.0, -0.002, 0.0).normalized();
        let mut prev = 0.0;
        for geo in [100.0, 500.0, 1500.0, 3000.0, 6000.0] {
            let a = day.effect(d, geo).map(|e| e.alpha).unwrap_or(0.0);
            assert!(a >= prev, "geo {geo}");
            prev = a;
        }
        assert!(prev > 0.2);
    }

    #[test]
    fn limb_is_brighter_on_lit_side() {
        // desde el espacio, rayo tangente: lado iluminado vs oscuro
        let tangent = |sun: Vec3| {
            let b = AtmBody::new(for_kind(BodyKind::Verdalia).unwrap(), Vec3::new(0.0, 0.0, -20_000.0), 3500.0, sun);
            let ang = (3500.0f32 + 400.0) / 20_000.0;
            let d = Vec3::new(0.0, ang.tan(), -1.0).normalized();
            b.effect(d, f32::INFINITY).map(|e| e.alpha).unwrap_or(0.0)
        };
        assert!(tangent(Vec3::Y) > tangent(-Vec3::Y) + 0.2);
    }
}
