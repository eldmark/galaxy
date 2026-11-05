#![allow(dead_code)]

mod app;
mod geometry;
mod hud;
mod math;
mod platform;
mod png;
mod raster;
mod relief;
mod shading;
mod ship;
mod shot;
mod world;

use std::path::Path;

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let has = |f: &str| args.iter().any(|a| a == f);
    let time: f32 = arg_value(&args, "--time").and_then(|t| t.parse().ok()).unwrap_or(5.0);
    if has("--help") {
        println!(
            "galaxy [--tour]                 ventana interactiva (--tour inicia el piloto automático)\n\
             galaxy --bench [--frames N]     compara con/sin culling y LOD sin ventana\n\
             galaxy --record DIR [--frames N] [--every K]   guarda cuadros del Tour sin ventana\n\
             galaxy --shot out.png [--time T] [--stats]   vista del sistema\n\
             galaxy --shot-ship out.png      vista de la nave"
        );
        return;
    }
    if let Some(out) = arg_value(&args, "--shot") {
        shot::system(Path::new(&out), time, has("--stats")).expect("no se pudo guardar el PNG");
        return;
    }
    if let Some(out) = arg_value(&args, "--shot-ship") {
        shot::ship(Path::new(&out), time).expect("error al renderizar la nave");
        return;
    }
    if let Some(dir) = arg_value(&args, "--record") {
        let frames = arg_value(&args, "--frames").and_then(|n| n.parse().ok()).unwrap_or(1800);
        let every = arg_value(&args, "--every").and_then(|n| n.parse().ok()).unwrap_or(60);
        app::record(&dir, frames, every);
        return;
    }
    if has("--bench") {
        let frames = arg_value(&args, "--frames").and_then(|n| n.parse().ok()).unwrap_or(3000);
        app::bench(frames);
        return;
    }
    app::run(has("--tour"));
}
