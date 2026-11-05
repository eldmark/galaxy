//! Capturas sin ventana (`--shot`, `--shot-ship`): sirven para verificar el pipeline completo
//! (OBJ, LOD de esfera, shaders reales, rasterizador) sin depender de la navegación.
use crate::geometry::SphereLods;
use crate::shipmodel::ShipModel;
use crate::math::{Mat4, Vec3};
use crate::raster::{DrawCall, Framebuffer, RenderOptions, Stats, Uniforms, render_scene};
use crate::shading::{shader_for, shader_for_ring, ship_shader, sky_color};
use crate::world::solar_system;
use std::path::Path;

pub const WIDTH: usize = 960;
pub const HEIGHT: usize = 540;
const FOV_Y: f32 = 1.0;

fn paint_sky(fb: &mut Framebuffer, view: Mat4, time: f32) {
    let t = (FOV_Y * 0.5).tan();
    let aspect = fb.width as f32 / fb.height as f32;
    for y in 0..fb.height {
        for x in 0..fb.width {
            let nx = (x as f32 + 0.5) / fb.width as f32 * 2.0 - 1.0;
            let ny = 1.0 - (y as f32 + 0.5) / fb.height as f32 * 2.0;
            let d = Vec3::new(nx * aspect * t, ny * t, -1.0).normalized();
            // dirección de cámara a mundo: transpuesta de la parte rotacional de `view`
            let w = view.transpose().transform_dir(d);
            let c = sky_color(w, time);
            let i = (y * fb.width + x) * 4;
            for k in 0..3 {
                fb.color[i + k] = (c[k].clamp(0.0, 1.0) * 255.0) as u8;
            }
            fb.color[i + 3] = 255;
        }
    }
}

/// Vista del sistema: sol y planetas en fila con su LOD y shader reales.
pub fn system(out: &Path, time: f32, stats_out: bool) -> std::io::Result<()> {
    let bodies = solar_system();
    let lods = SphereLods::new(5, &|_| 0.0);
    let shaders: Vec<_> = bodies.iter().map(|b| shader_for(b.kind)).collect();
    let ring_shader = shader_for_ring();
    let ring = crate::geometry::ring_mesh(1.3, 2.1, 96);

    // Fila de cuerpos a lo largo de X con separación proporcional al radio.
    let mut x = 0.0f32;
    let mut centers = Vec::new();
    for b in &bodies {
        x += b.radius as f32 * 1.35;
        centers.push(x);
        x += b.radius as f32 * 1.35;
    }
    let total = x;
    let eye = Vec3::new(total * 0.5, total * 0.16, total * 0.62);
    let target = Vec3::new(total * 0.5, 0.0, 0.0);
    let view = Mat4::look_at(eye, target, Vec3::Y);
    let proj = Mat4::perspective(FOV_Y, WIDTH as f32 / HEIGHT as f32, 0.5, 500_000.0);

    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    paint_sky(&mut fb, view, time);
    let sun_pos = Vec3::new(centers[0], 0.0, 0.0);
    let mut calls: Vec<DrawCall> = Vec::new();
    for (i, b) in bodies.iter().enumerate() {
        let c = Vec3::new(centers[i], 0.0, 0.0);
        let r = b.radius as f32;
        let dist = (c - eye).length();
        let level = lods.select(dist, r, FOV_Y, HEIGHT as f32);
        let spin = time * std::f32::consts::TAU / b.rotation_period as f32;
        let model = Mat4::translation(c) * Mat4::rotation_y(spin) * Mat4::uniform_scale(r);
        let light = if i == 0 { Vec3::Y } else { (sun_pos - c).normalized() };
        calls.push(DrawCall {
            mesh: &lods.levels[level],
            model,
            shader: shaders[i].as_ref(),
            uniforms: Uniforms { light_dir: light, time, detail: 1.0, model },
        });
        if b.name == "Siroco" {
            let m = Mat4::translation(c) * Mat4::rotation_z(0.35) * Mat4::uniform_scale(r);
            calls.push(DrawCall {
                mesh: &ring,
                model: m,
                shader: ring_shader.as_ref(),
                uniforms: Uniforms { light_dir: light, time, detail: 1.0, model: m },
            });
        }
    }
    let mut stats = Stats::default();
    render_scene(&mut fb, view, proj, &calls, &RenderOptions::default(), &mut stats);
    fb.save_png(out)?;
    if stats_out {
        println!("{stats:#?}");
    }
    Ok(())
}

/// Vista cercana de la nave real (`assets/models/ship.obj`) con el tren en `gear` (0 plegado, 1 desplegado).
pub fn ship(out: &Path, time: f32, gear: f32) -> Result<(), Box<dyn std::error::Error>> {
    let sm = ShipModel::load("assets/models/ship.obj")?;
    let shader = ship_shader();
    let eye = Vec3::new(14.0, 7.0, -20.0);
    let view = Mat4::look_at(eye, Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective(FOV_Y, WIDTH as f32 / HEIGHT as f32, 0.5, 500_000.0);
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    paint_sky(&mut fb, view, time);
    let model = Mat4::rotation_y(time * 0.5);
    let light = Vec3::new(0.5, 0.8, -0.3).normalized();
    let uniforms = Uniforms { light_dir: light, time, detail: 1.0, model };
    let calls = sm.draw_calls(model, gear, shader.as_ref(), uniforms);
    let mut stats = Stats::default();
    render_scene(&mut fb, view, proj, &calls,&RenderOptions::default(), &mut stats);
    fb.save_png(out)?;
    Ok(())
}
