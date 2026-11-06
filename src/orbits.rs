//! Líneas de órbita: la órbita circular inclinada de cada planeta como polilínea de 192 segmentos, proyectada con
//! nuestro propio pipeline (matriz de vista/proyección de la cámara), recortada contra el plano cercano y la
//! pantalla, con prueba de profundidad contra `fb.depth` (1/w) y líneas de 1 px antialias (Xiaolin Wu) propias.
use crate::math::{DVec3, Mat4, Vec3};
use crate::raster::Framebuffer;
use crate::world::{Body, BodyKind};

pub const ORBIT_SEGMENTS: usize = 192;
const NEAR: f32 = 1.0;

/// Color tenue de la línea de cada planeta.
pub fn orbit_color(kind: BodyKind) -> [u8; 3] {
    match kind {
        BodyKind::Sun => [255, 200, 120],
        BodyKind::Fornax => [255, 140, 90],
        BodyKind::Verdalia => [110, 220, 150],
        BodyKind::Mutante => [200, 130, 240],
        BodyKind::Siroco => [240, 210, 140],
        BodyKind::Nix => [140, 210, 255],
    }
}

/// Recorta el segmento (en espacio de cámara, mira -Z) contra el plano cercano `z <= -near`.
pub fn clip_near(a: Vec3, b: Vec3, near: f32) -> Option<(Vec3, Vec3)> {
    let (da, db) = (-a.z - near, -b.z - near); // >= 0 = dentro
    if da < 0.0 && db < 0.0 {
        return None;
    }
    if da >= 0.0 && db >= 0.0 {
        return Some((a, b));
    }
    let t = da / (da - db);
    let p = a + (b - a) * t;
    Some(if da < 0.0 { (p, b) } else { (a, p) })
}

/// Proyecta un punto de espacio de cámara a (x, y en píxeles, 1/w). `None` si está detrás del plano cercano.
pub fn project(proj: &Mat4, p: Vec3, w: usize, h: usize) -> Option<(f32, f32, f32)> {
    let c = proj.transform_point4(p);
    if !(c[3] > 1e-6) {
        return None;
    }
    let sx = (c[0] / c[3] * 0.5 + 0.5) * w as f32;
    let sy = (1.0 - (c[1] / c[3] * 0.5 + 0.5)) * h as f32;
    (sx.is_finite() && sy.is_finite()).then_some((sx, sy, 1.0 / c[3]))
}

/// Liang-Barsky: parte del segmento (x0,y0)-(x1,y1) dentro de [0,w]x[0,h] como (t0, t1).
pub fn clip_rect(x0: f32, y0: f32, x1: f32, y1: f32, w: f32, h: f32) -> Option<(f32, f32)> {
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [(-dx, x0), (dx, w - x0), (-dy, y0), (dy, h - y0)] {
        if p.abs() < 1e-9 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    (t0 < t1).then_some((t0, t1))
}

fn plot(fb: &mut Framebuffer, x: i32, y: i32, cov: f32, iw: f32, rgb: [u8; 3], alpha: f32) {
    if x < 0 || y < 0 || x >= fb.width as i32 || y >= fb.height as i32 {
        return;
    }
    let i = y as usize * fb.width + x as usize;
    if iw < fb.depth[i] * 0.998 {
        return; // tapada por geometría más cercana
    }
    let a = (cov * alpha).clamp(0.0, 1.0);
    let o = i * 4;
    for k in 0..3 {
        let c = fb.color[o + k] as f32;
        fb.color[o + k] = (c + (rgb[k] as f32 - c) * a) as u8;
    }
}

/// Línea de 1 px con mezcla suave (Wu) e interpolación lineal de 1/w. Recorta a la pantalla.
pub fn draw_line(fb: &mut Framebuffer, p0: (f32, f32, f32), p1: (f32, f32, f32), rgb: [u8; 3], alpha: f32) {
    let (w, h) = (fb.width as f32, fb.height as f32);
    let Some((t0, t1)) = clip_rect(p0.0, p0.1, p1.0, p1.1, w - 1.0, h - 1.0) else { return };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let (mut x0, mut y0, mut z0) = (lerp(p0.0, p1.0, t0), lerp(p0.1, p1.1, t0), lerp(p0.2, p1.2, t0));
    let (mut x1, mut y1, mut z1) = (lerp(p0.0, p1.0, t1), lerp(p0.1, p1.1, t1), lerp(p0.2, p1.2, t1));
    let steep = (y1 - y0).abs() > (x1 - x0).abs();
    if steep {
        std::mem::swap(&mut x0, &mut y0);
        std::mem::swap(&mut x1, &mut y1);
    }
    if x0 > x1 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
        std::mem::swap(&mut z0, &mut z1);
    }
    let dx = x1 - x0;
    let grad = if dx.abs() < 1e-6 { 0.0 } else { (y1 - y0) / dx };
    let xs = x0.floor() as i32;
    let xe = x1.floor() as i32;
    for x in xs..=xe {
        let t = if dx.abs() < 1e-6 { 0.0 } else { ((x as f32 + 0.5 - x0) / dx).clamp(0.0, 1.0) };
        let y = y0 + grad * (x as f32 + 0.5 - x0);
        let iw = lerp(z0, z1, t);
        let yi = (y - 0.5).floor();
        let f = y - 0.5 - yi;
        let (a, b) = (yi as i32, yi as i32 + 1);
        if steep {
            plot(fb, a, x, 1.0 - f, iw, rgb, alpha);
            plot(fb, b, x, f, iw, rgb, alpha);
        } else {
            plot(fb, x, a, 1.0 - f, iw, rgb, alpha);
            plot(fb, x, b, f, iw, rgb, alpha);
        }
    }
}

/// Dibuja una polilínea cerrada de puntos del mundo (f64) con el pipeline de la cámara.
pub fn draw_polyline(fb: &mut Framebuffer, view: &Mat4, proj: &Mat4, eye: DVec3, pts: &[DVec3], rgb: [u8; 3], alpha: f32) {
    let (w, h) = (fb.width, fb.height);
    let cam: Vec<Vec3> = pts.iter().map(|p| view.transform_point((*p - eye).to_f32())).collect();
    for i in 0..cam.len() {
        let (a, b) = (cam[i], cam[(i + 1) % cam.len()]);
        let Some((a, b)) = clip_near(a, b, NEAR) else { continue };
        let (Some(pa), Some(pb)) = (project(proj, a, w, h), project(proj, b, w, h)) else { continue };
        draw_line(fb, pa, pb, rgb, alpha);
    }
}

/// Puntos de la órbita de un cuerpo (independientes de `t`: la posición es función analítica del tiempo).
pub fn orbit_points(b: &Body, n: usize) -> Vec<DVec3> {
    (0..n).map(|k| b.position(b.orbit_period * k as f64 / n as f64)).collect()
}

/// Dibuja las órbitas de todos los cuerpos con órbita (coste < 1 ms a 960x540).
pub fn draw_orbits(fb: &mut Framebuffer, view: Mat4, proj: Mat4, eye: DVec3, bodies: &[Body], _t: f64) {
    for b in bodies {
        if b.orbit_radius <= 0.0 || b.orbit_period <= 0.0 {
            continue;
        }
        draw_polyline(fb, &view, &proj, eye, &orbit_points(b, ORBIT_SEGMENTS), orbit_color(b.kind), 0.5);
    }
}

/// Cuerpo de referencia orbital más cercano y datos para el HUD de órbita.
#[derive(Clone, Copy, Debug)]
pub struct OrbitRef {
    pub body: usize,
    /// Distancia al centro (m) y altitud sobre la esfera (m).
    pub r: f64,
    pub altitude: f64,
    /// Velocidad relativa al cuerpo (m/s) y componente radial.
    pub v_rel: f64,
    pub v_radial: f64,
    /// Velocidad circular de referencia (m/s).
    pub v_circ: f64,
    /// |v - vc| < 10 % y altitud ~constante (|vr| pequeña).
    pub stable: bool,
}

pub fn orbit_ref(bodies: &[Body], pos: DVec3, vel: DVec3, t: f64) -> Option<OrbitRef> {
    let mut best: Option<(usize, f64)> = None;
    for (i, b) in bodies.iter().enumerate() {
        if b.gm <= 0.0 {
            continue;
        }
        let r = (pos - b.position(t)).length();
        if r < b.influence_radius() && best.map_or(true, |(_, br)| r / b.influence_radius() < br) {
            best = Some((i, r / b.influence_radius()));
        }
    }
    let (i, _) = best?;
    let b = &bodies[i];
    let rel = pos - b.position(t);
    let r = rel.length();
    let v = vel - b.velocity(t);
    let vr = v.dot(rel) / r.max(1e-9);
    let vc = b.circular_speed(r);
    let vs = v.length();
    Some(OrbitRef {
        body: i,
        r,
        altitude: r - b.radius,
        v_rel: vs,
        v_radial: vr,
        v_circ: vc,
        stable: vc > 1.0 && r > b.radius * 1.05 && (vs - vc).abs() < 0.1 * vc && vr.abs() < 0.08 * vc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::solar_system;

    fn proj() -> Mat4 {
        Mat4::perspective(1.0, 960.0 / 540.0, NEAR, 600_000.0)
    }

    #[test]
    fn near_clip_cuts_at_plane() {
        let (a, b) = (Vec3::new(0.0, 0.0, -10.0), Vec3::new(0.0, 0.0, 10.0));
        let (p, q) = clip_near(a, b, 1.0).unwrap();
        assert!((p.z + 1.0).abs() < 1e-5 && q.z == -10.0 || (q.z + 1.0).abs() < 1e-5);
        assert!(clip_near(Vec3::new(0.0, 0.0, 5.0), Vec3::new(1.0, 0.0, 2.0), 1.0).is_none());
        assert!(clip_near(Vec3::new(0.0, 0.0, -5.0), Vec3::new(1.0, 0.0, -2.0), 1.0).is_some());
    }

    #[test]
    fn projection_center_and_behind() {
        let (x, y, iw) = project(&proj(), Vec3::new(0.0, 0.0, -100.0), 960, 540).unwrap();
        assert!((x - 480.0).abs() < 1e-3 && (y - 270.0).abs() < 1e-3);
        assert!((iw - 0.01).abs() < 1e-4);
        assert!(project(&proj(), Vec3::new(0.0, 0.0, 10.0), 960, 540).is_none());
    }

    #[test]
    fn rect_clip() {
        assert!(clip_rect(-100.0, -100.0, -50.0, -10.0, 100.0, 100.0).is_none());
        let (t0, t1) = clip_rect(-50.0, 50.0, 150.0, 50.0, 100.0, 100.0).unwrap();
        assert!((t0 - 0.25).abs() < 1e-5 && (t1 - 0.75).abs() < 1e-5);
    }

    #[test]
    fn line_draws_and_respects_depth() {
        let mut fb = Framebuffer::new(64, 64);
        draw_line(&mut fb, (5.0, 5.0, 0.1), (50.0, 40.0, 0.1), [255, 255, 255], 1.0);
        let lit = fb.color.chunks(4).filter(|p| p[0] > 0).count();
        assert!(lit > 40, "{lit}");
        // un oclusor más cercano (mayor 1/w) tapa la línea
        let mut fb = Framebuffer::new(64, 64);
        fb.depth.fill(1.0);
        draw_line(&mut fb, (5.0, 5.0, 0.1), (50.0, 40.0, 0.1), [255, 255, 255], 1.0);
        assert_eq!(fb.color.chunks(4).filter(|p| p[0] > 0).count(), 0);
        // enorme y fuera de pantalla: no se cuelga
        let mut fb = Framebuffer::new(64, 64);
        draw_line(&mut fb, (-1e7, 30.0, 0.1), (1e7, 31.0, 0.1), [255, 255, 255], 1.0);
        assert!(fb.color.chunks(4).filter(|p| p[0] > 0).count() > 60);
    }

    #[test]
    fn orbits_draw_from_above_and_are_fast() {
        let bodies = solar_system();
        let mut fb = Framebuffer::new(960, 540);
        // cámara a 300 km sobre el sol mirando hacia abajo (-Y)
        let eye = DVec3::new(0.0, 300_000.0, 0.0);
        let view = Mat4::look_at(Vec3::ZERO, Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, -1.0));
        let t0 = std::time::Instant::now();
        for _ in 0..20 {
            draw_orbits(&mut fb, view, proj(), eye, &bodies, 0.0);
        }
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / 20.0;
        println!("draw_orbits: {ms:.3} ms");
        let lit = fb.color.chunks(4).filter(|p| p[0] > 0 || p[1] > 0).count();
        assert!(lit > 500, "{lit}");
        assert!(ms < 5.0);
    }

    #[test]
    fn orbit_ref_reports_circular_speed_and_stability() {
        let bodies = solar_system();
        let b = &bodies[2];
        let r = 1.5 * b.radius;
        let vc = b.circular_speed(r);
        let pos = b.position(0.0) + DVec3::new(0.0, 0.0, r);
        let o = orbit_ref(&bodies, pos, b.velocity(0.0) + DVec3::new(vc, 0.0, 0.0), 0.0).unwrap();
        assert_eq!(o.body, 2);
        assert!(o.stable && (o.v_circ - vc).abs() < 1e-9);
        let o = orbit_ref(&bodies, pos, b.velocity(0.0) + DVec3::new(vc * 1.3, 0.0, 0.0), 0.0).unwrap();
        assert!(!o.stable);
        let o = orbit_ref(&bodies, pos, b.velocity(0.0) + DVec3::new(vc, 0.0, vc * 0.5), 0.0).unwrap();
        assert!(!o.stable, "con velocidad radial no es estable");
    }
}
