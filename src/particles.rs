//! Partículas simples (puntos/discos emisivos proyectados con nuestro pipeline) y pasadas de pantalla propias:
//! polvo/chispas al tocar y despegar, explosión de la nave y tinte de calor / destello / fundido.
//! Todo determinista (hash de enteros, sin reloj ni aleatoriedad del sistema).
use crate::math::{DVec3, Mat4, Vec3};
use crate::orbits::project;
use crate::raster::Framebuffer;
use crate::world::{Body, BodyKind};

/// Hash determinista 0..1 a partir de dos enteros.
pub fn hash01(a: u32, b: u32) -> f32 {
    let mut x = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B).rotate_left(13);
    x ^= x >> 16;
    x = x.wrapping_mul(0x7FEB_352D);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846C_A68B);
    x ^= x >> 16;
    (x >> 8) as f32 / 16_777_216.0
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Bola de fuego: blanco -> naranja -> rojo, mezcla aditiva; al final humo.
    Fire,
    /// Polvo/ceniza/nieve con color propio, mezcla normal.
    Dust,
    /// Chispa brillante, aditiva.
    Spark,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    /// Cuerpo cuyo marco (fijo a él, gira y se traslada con él) usan `p` y `v`; `None` = marco del mundo.
    pub body: Option<usize>,
    pub p: DVec3,
    pub v: DVec3,
    pub age: f32,
    pub life: f32,
    /// Radio en metros.
    pub size: f32,
    pub kind: Kind,
    pub color: [u8; 3],
}

#[derive(Default)]
pub struct Particles {
    pub list: Vec<Particle>,
    counter: u32,
}

/// Color del polvo según el cuerpo: arena (Verdalia, Mutante), ceniza (Fornax), nieve (Nix).
pub fn dust_color(kind: BodyKind) -> [u8; 3] {
    match kind {
        BodyKind::Verdalia => [206, 178, 128],
        BodyKind::Mutante => [190, 140, 170],
        BodyKind::Fornax => [96, 88, 86],
        BodyKind::Nix => [232, 242, 255],
        _ => [180, 180, 180],
    }
}

impl Particles {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.list.len()
    }
    pub fn clear(&mut self) {
        self.list.clear();
    }

    fn rnd(&mut self, seed: u32) -> f32 {
        self.counter = self.counter.wrapping_add(1);
        hash01(seed, self.counter)
    }

    fn rnd_dir(&mut self, seed: u32) -> DVec3 {
        let z = self.rnd(seed) * 2.0 - 1.0;
        let a = self.rnd(seed) * std::f32::consts::TAU;
        let r = (1.0 - z * z).max(0.0).sqrt();
        DVec3::new((r * a.cos()) as f64, (r * a.sin()) as f64, z as f64)
    }

    /// Explosión: `n` partículas (150-300) en `pos` con velocidad base `vel` (marco `body` o mundo).
    pub fn emit_explosion(&mut self, body: Option<usize>, pos: DVec3, vel: DVec3, n: usize, seed: u32) {
        for _ in 0..n {
            let d = self.rnd_dir(seed);
            let spark = self.rnd(seed) < 0.25;
            let speed = (8.0 + 110.0 * self.rnd(seed).powf(1.5) as f64) * if spark { 2.5 } else { 1.0 };
            let life = 1.5 + 2.0 * self.rnd(seed);
            let part = Particle {
                body,
                p: pos + d * 6.0,
                v: vel * 0.2 + d * speed,
                age: 0.0,
                life: if spark { life * 0.7 } else { life },
                size: if spark { 3.0 + 4.0 * self.rnd(seed) } else { 45.0 + 70.0 * self.rnd(seed) },
                kind: if spark { Kind::Spark } else { Kind::Fire },
                color: [255, 255, 255],
            };
            self.list.push(part);
        }
    }

    /// Polvo en el suelo: `n` partículas en `pos` (coordenadas fijas al cuerpo) sobre la normal `up`.
    /// `strength` 0..1 escala la velocidad de salida.
    pub fn emit_dust(&mut self, body: usize, pos: DVec3, up: DVec3, color: [u8; 3], n: usize, strength: f32, seed: u32) {
        let t1 = if up.y.abs() < 0.9 { DVec3::new(0.0, 1.0, 0.0) } else { DVec3::new(1.0, 0.0, 0.0) };
        let e1 = crate::world::cross(up, t1).normalized();
        let e2 = crate::world::cross(up, e1).normalized();
        for _ in 0..n {
            let a = self.rnd(seed) * std::f32::consts::TAU;
            let ring = 2.0 + 14.0 * self.rnd(seed) as f64;
            let dir = e1 * a.cos() as f64 + e2 * a.sin() as f64;
            let spd = (6.0 + 26.0 * self.rnd(seed) as f64) * strength.max(0.2) as f64;
            let jitter = 0.8 + 0.4 * self.rnd(seed);
            let part = Particle {
                body: Some(body),
                p: pos + dir * ring + up * 0.5,
                v: dir * spd + up * (1.5 + 5.0 * self.rnd(seed) as f64),
                age: 0.0,
                life: 1.2 + 1.6 * self.rnd(seed),
                size: 2.5 + 4.5 * self.rnd(seed),
                kind: if self.rnd(seed) < 0.12 { Kind::Spark } else { Kind::Dust },
                color: color.map(|c| (c as f32 * jitter).min(255.0) as u8),
            };
            self.list.push(part);
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.list {
            p.age += dt;
            let drag = (-(if p.kind == Kind::Fire { 0.9 } else { 1.6 }) * dt as f64).exp();
            p.v = p.v * drag;
            p.p = p.p + p.v * dt as f64;
        }
        self.list.retain(|p| p.age < p.life);
    }

    /// Posición de la partícula en el mundo (f64).
    fn world_pos(p: &Particle, bodies: &[Body], t: f64) -> DVec3 {
        match p.body {
            Some(i) => bodies[i].position(t) + bodies[i].fixed_to_world(t, p.p),
            None => p.p,
        }
    }

    /// Dibuja todas las partículas con profundidad contra `fb.depth`.
    pub fn draw(&self, fb: &mut Framebuffer, view: &Mat4, proj: &Mat4, eye: DVec3, bodies: &[Body], t: f64) {
        let (w, h) = (fb.width, fb.height);
        let f = proj.m[1][1] * h as f32 * 0.5;
        for p in &self.list {
            let rel = (Self::world_pos(p, bodies, t) - eye).to_f32();
            let pv = view.transform_point(rel);
            if pv.z > -1.0 {
                continue;
            }
            let Some((sx, sy, iw)) = project(proj, pv, w, h) else { continue };
            let f_life = p.age / p.life;
            let grow = match p.kind {
                Kind::Fire => 0.6 + 0.9 * f_life,
                Kind::Dust => 0.7 + 1.2 * f_life,
                Kind::Spark => 1.0 - 0.5 * f_life,
            };
            let r = (p.size * grow * f * iw).clamp(0.8, 70.0);
            let (rgb, alpha, additive) = shade(p, f_life);
            draw_disc(fb, sx, sy, r, iw, rgb, alpha, additive);
        }
    }
}

/// Color, opacidad y modo de mezcla de una partícula según su edad relativa.
fn shade(p: &Particle, f: f32) -> ([u8; 3], f32, bool) {
    let mix = |a: [f32; 3], b: [f32; 3], t: f32| [0, 1, 2].map(|i| (a[i] + (b[i] - a[i]) * t.clamp(0.0, 1.0)) as u8);
    match p.kind {
        Kind::Fire => {
            if f < 0.15 {
                (mix([255.0, 255.0, 240.0], [255.0, 190.0, 70.0], f / 0.15), 0.95, false)
            } else if f < 0.5 {
                (mix([255.0, 190.0, 70.0], [170.0, 45.0, 15.0], (f - 0.15) / 0.35), 0.9 * (1.0 - (f - 0.15) * 0.9), false)
            } else {
                // humo: gris oscuro que se desvanece, mezcla normal
                (mix([90.0, 45.0, 30.0], [50.0, 48.0, 48.0], (f - 0.5) / 0.3), (1.0 - f).max(0.0) * 1.1, false)
            }
        }
        Kind::Spark => (mix([255.0, 255.0, 220.0], [255.0, 120.0, 30.0], f), 1.0 - f, true),
        Kind::Dust => (p.color, 0.55 * (1.0 - f) * (1.0 - f), false),
    }
}

/// Disco suave de radio `r` px con caída radial; `additive` suma luz en vez de mezclar.
pub fn draw_disc(fb: &mut Framebuffer, cx: f32, cy: f32, r: f32, iw: f32, rgb: [u8; 3], alpha: f32, additive: bool) {
    let (w, h) = (fb.width as i32, fb.height as i32);
    let x0 = ((cx - r).floor() as i32).max(0);
    let x1 = ((cx + r).ceil() as i32).min(w - 1);
    let y0 = ((cy - r).floor() as i32).max(0);
    let y1 = ((cy + r).ceil() as i32).min(h - 1);
    let inv_r2 = 1.0 / (r * r).max(1e-6);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
            let d2 = (dx * dx + dy * dy) * inv_r2;
            if d2 >= 1.0 {
                continue;
            }
            let i = y as usize * fb.width + x as usize;
            if iw < fb.depth[i] * 0.998 {
                continue;
            }
            let a = alpha * (1.0 - d2) * (1.0 - d2) * if additive { 1.0 } else { 1.3 };
            let o = i * 4;
            for k in 0..3 {
                let c = fb.color[o + k] as f32;
                let v = if additive { c + rgb[k] as f32 * a } else { c + (rgb[k] as f32 - c) * a.min(1.0) };
                fb.color[o + k] = v.clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Mezcla un color sobre todo el framebuffer con opacidad `alpha` (destello blanco, fundido a negro).
pub fn fade(fb: &mut Framebuffer, rgb: [u8; 3], alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    if a <= 0.0 {
        return;
    }
    let k = (a * 256.0) as i32;
    for px in fb.color.chunks_exact_mut(4) {
        for c in 0..3 {
            let v = px[c] as i32;
            px[c] = (v + (((rgb[c] as i32 - v) * k) >> 8)).clamp(0, 255) as u8;
        }
    }
}

/// Tinte de calor sobre la imagen: viñeta rojiza que crece con `heat` y, cerca de 1, lavado blanco.
pub fn heat_tint(fb: &mut Framebuffer, heat: f32) {
    let heat = heat.clamp(0.0, 1.0);
    if heat < 0.03 {
        return;
    }
    let (w, h) = (fb.width, fb.height);
    let white = ((heat - 0.6) / 0.4).clamp(0.0, 1.0);
    for y in 0..h {
        let ny = (y as f32 + 0.5) / h as f32 * 2.0 - 1.0;
        for x in 0..w {
            let nx = (x as f32 + 0.5) / w as f32 * 2.0 - 1.0;
            let edge = ((nx * nx + ny * ny) * 0.5).min(1.0);
            let a = heat * (0.10 + 0.55 * edge * edge) + white * 0.35;
            let a = a.min(0.9);
            let tint = if white > 0.0 { [255.0, 225.0 + 30.0 * white, 190.0 + 65.0 * white] } else { [255.0, 70.0, 20.0] };
            let o = (y * w + x) * 4;
            for c in 0..3 {
                let v = fb.color[o + c] as f32;
                fb.color[o + c] = (v + (tint[c] - v) * a) as u8;
            }
        }
    }
}

/// Opacidad del destello blanco a los `age` s de la destrucción: sube en 0.06 s y decae hasta ~0.9 s.
pub fn flash_alpha(age: f64) -> f32 {
    if age < 0.0 {
        0.0
    } else if age < 0.06 {
        (age / 0.06) as f32
    } else {
        (1.0 - (age - 0.06) / 0.85).clamp(0.0, 1.0) as f32
    }
}

/// Opacidad del fundido a negro: empieza a los 2.6 s y llega a 1 a los 4.0 s (el instante de reaparición).
pub fn death_fade_alpha(age: f64) -> f32 {
    ((age - 2.6) / 1.4).clamp(0.0, 1.0) as f32
}

/// Opacidad del fundido de entrada tras reaparecer (1 -> 0 en 1 s).
pub fn respawn_fade_alpha(since_respawn: f64) -> f32 {
    (1.0 - since_respawn).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::solar_system;

    #[test]
    fn hash_is_deterministic_and_spread() {
        assert_eq!(hash01(3, 7), hash01(3, 7));
        let mean: f32 = (0..1000).map(|i| hash01(5, i)).sum::<f32>() / 1000.0;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
        assert!((0..1000).all(|i| (0.0..1.0).contains(&hash01(1, i))));
    }

    #[test]
    fn explosion_is_deterministic_and_expires() {
        let mk = || {
            let mut p = Particles::new();
            p.emit_explosion(None, DVec3::new(1.0, 2.0, 3.0), DVec3::ZERO, 240, 7);
            p
        };
        let (a, b) = (mk(), mk());
        assert_eq!(a.len(), 240);
        assert!(a.list.iter().zip(&b.list).all(|(x, y)| x.p == y.p && x.v == y.v));
        let mut a = a;
        for _ in 0..(4 * 60) {
            a.update(1.0 / 60.0);
        }
        assert_eq!(a.len(), 0, "todas deben desvanecerse");
    }

    #[test]
    fn dust_follows_planet_frame_and_has_planet_color() {
        let mut p = Particles::new();
        p.emit_dust(2, DVec3::new(0.0, 3500.0, 0.0), DVec3::new(0.0, 1.0, 0.0), dust_color(BodyKind::Verdalia), 50, 1.0, 1);
        assert_eq!(p.len(), 50);
        assert!(p.list.iter().all(|q| q.body == Some(2)));
        assert_ne!(dust_color(BodyKind::Nix), dust_color(BodyKind::Fornax));
    }

    #[test]
    fn particles_draw_in_front_of_sky_and_not_behind_depth() {
        let bodies = solar_system();
        let proj = Mat4::perspective(1.0, 960.0 / 540.0, 1.0, 600_000.0);
        let view = Mat4::IDENTITY;
        let mut ps = Particles::new();
        ps.emit_explosion(None, DVec3::new(0.0, 0.0, -100.0), DVec3::ZERO, 200, 3);
        let mut fb = Framebuffer::new(960, 540);
        ps.draw(&mut fb, &view, &proj, DVec3::ZERO, &bodies, 0.0);
        let lit = fb.color.chunks(4).filter(|p| p[0] > 20).count();
        assert!(lit > 500, "{lit}");
        // con un oclusor cercano delante de todo no se dibuja nada
        let mut fb = Framebuffer::new(960, 540);
        fb.depth.fill(10.0);
        ps.draw(&mut fb, &view, &proj, DVec3::ZERO, &bodies, 0.0);
        assert_eq!(fb.color.chunks(4).filter(|p| p[0] > 0).count(), 0);
    }

    #[test]
    fn overlays_behave() {
        let mut fb = Framebuffer::new(32, 32);
        fb.clear([10, 10, 10]);
        heat_tint(&mut fb, 0.0);
        assert_eq!(fb.color[0], 10);
        heat_tint(&mut fb, 0.5);
        assert!(fb.color[0] > 10 && fb.color[2] < fb.color[0]);
        fade(&mut fb, [255, 255, 255], 1.0);
        assert!(fb.color[0] >= 254 && fb.color[1] >= 254);
        fade(&mut fb, [0, 0, 0], 1.0);
        assert!(fb.color[0] <= 1);
        assert!(flash_alpha(0.06) > 0.99 && flash_alpha(2.0) == 0.0);
        assert_eq!(death_fade_alpha(1.0), 0.0);
        assert_eq!(death_fade_alpha(4.0), 1.0);
    }
}
