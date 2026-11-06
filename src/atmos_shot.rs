//! `--atmos-shot out.png --body verdalia --alt 40 [--sun GRADOS] [--space] [--no-normalmap]`
//!
//! Construye un `Sim` con la nave a la altitud pedida sobre el relieve (como `app::new_sim`), mirando al
//! horizonte, y llama a `Scene::render` real (terreno + atmósfera + mapa de normales). `--sun` es la elevación
//! del sol sobre el horizonte local en grados (poca = luz rasante); `--space` mira hacia el borde del planeta.
use crate::app::{HEIGHT, Scene, WIDTH, new_sim};
use crate::math::{DVec3, Quat, Vec3};
use crate::raster::{Framebuffer, RenderOptions, Stats};
use crate::ship::CameraRig;
use crate::world::{BodyKind, surface_height_at};
use std::path::Path;
use std::time::Instant;

/// Cuaternión de la rotación cuyas columnas son (derecha, arriba, atrás).
fn quat_from_basis(r: Vec3, u: Vec3, b: Vec3) -> Quat {
    let (m00, m01, m02, m10, m11, m12, m20, m21, m22) = (r.x, u.x, b.x, r.y, u.y, b.y, r.z, u.z, b.z);
    let tr = m00 + m11 + m22;
    let q = if tr > 0.0 {
        let s = (tr + 1.0).sqrt() * 2.0;
        Quat { w: 0.25 * s, x: (m21 - m12) / s, y: (m02 - m20) / s, z: (m10 - m01) / s }
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        Quat { w: (m21 - m12) / s, x: 0.25 * s, y: (m01 + m10) / s, z: (m02 + m20) / s }
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        Quat { w: (m02 - m20) / s, x: (m01 + m10) / s, y: 0.25 * s, z: (m12 + m21) / s }
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        Quat { w: (m10 - m01) / s, x: (m02 + m20) / s, y: (m12 + m21) / s, z: 0.25 * s }
    };
    q.normalized()
}

pub fn kind_from_name(n: &str) -> BodyKind {
    match n.to_lowercase().as_str() {
        "fornax" => BodyKind::Fornax,
        "mutante" => BodyKind::Mutante,
        "nix" => BodyKind::Nix,
        "siroco" => BodyKind::Siroco,
        _ => BodyKind::Verdalia,
    }
}

pub fn shot(out: &Path, body_name: &str, alt: f32, sun_deg: f32, az_deg: f32, normalmap: bool, space: bool) -> std::io::Result<()> {
    crate::shading::set_normal_map(normalmap);
    let kind = kind_from_name(body_name);
    let mut sim = new_sim();
    let idx = sim.bodies.iter().position(|b| b.kind == kind).expect("cuerpo desconocido");
    let body = sim.bodies[idx].clone();
    let pos = body.position(sim.t);
    let sun = (-pos).normalized();
    // Punto del suelo con el sol a `sun_deg` sobre el horizonte, desplazado hacia un lado del plano sol-eje.
    let side = DVec3::new(0.0, 1.0, 0.0);
    let perp = {
        let c = crate::world::cross(sun, side);
        c.normalized()
    };
    let e = (sun_deg as f64).to_radians();
    let a = (az_deg as f64).to_radians();
    let perp = perp * a.cos() + crate::world::cross(sun, perp) * a.sin();
    let dir = (sun * e.sin() + perp * e.cos()).normalized();
    let ground = surface_height_at(&body, sim.t, dir);
    sim.ship.position = pos + dir * (ground + alt as f64);
    sim.ship.velocity = body.velocity(sim.t);

    // Base local: arriba = dir; hacia el sol (horizontal); mirando perpendicular a él (luz rasante lateral).
    let up = dir.to_f32();
    let sun_h = {
        let s = sun.to_f32();
        (s - up * s.dot(up)).normalized()
    };
    let right_h = sun_h; // el sol queda a la derecha de la imagen
    let fwd_h = up.cross(right_h).normalized(); // horizontal, hacia delante
    let dip = (body.radius / (body.radius + alt as f64)).acos() as f32;
    let pitch = if space { -dip } else { -(dip * 0.75).max(0.1) };
    let fwd = fwd_h * pitch.cos() + up * pitch.sin();
    let right = fwd.cross(up).normalized();
    let cam_up = right.cross(fwd).normalized();
    sim.ship.orientation = quat_from_basis(right, cam_up, -fwd);

    let mut cam = CameraRig::new();
    cam.snap(&sim.ship);
    let mut scene = Scene::new();
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    let opts = RenderOptions::default();
    // Varios cuadros para que el terreno genere todos sus chunks (presupuesto por cuadro).
    let mut stats = Stats::default();
    let mut ms = 0.0;
    for _ in 0..60 {
        stats = Stats::default();
        let t0 = Instant::now();
        scene.render(&mut fb, &sim, &cam, &opts, true, &mut stats);
        ms = t0.elapsed().as_secs_f32() * 1000.0;
    }
    // Costo de la pasada atmosférica sola (sobre un cuadro ya dibujado).
    let view = cam.view_matrix();
    let eye = cam.eye(&sim.ship);
    let t1 = Instant::now();
    let n = 20;
    for _ in 0..n {
        crate::atmosphere::apply(&mut fb, view, crate::app::FOV_Y, eye, &sim.bodies, sim.t);
    }
    let ms_atm = t1.elapsed().as_secs_f32() * 1000.0 / n as f32;
    // La repetición de `apply` arruina la imagen: se vuelve a dibujar un cuadro limpio para guardarla.
    scene.render(&mut fb, &sim, &cam, &opts, true, &mut stats);
    fb.save_png(out)?;
    println!(
        "{} alt {alt} m sol {sun_deg} deg normalmap {} | cuadro {ms:.1} ms (atmósfera aparte: {ms_atm:.2} ms) | terreno {} tris",
        body.name,
        if normalmap { "on" } else { "off" },
        scene.terrain_triangles
    );
    Ok(())
}
