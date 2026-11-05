#![allow(dead_code)]

mod geometry;
mod hud;
mod math;
mod platform;
mod png;
mod raster;
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
    let time: f32 = arg_value(&args, "--time").and_then(|t| t.parse().ok()).unwrap_or(5.0);
    if let Some(out) = arg_value(&args, "--shot") {
        shot::system(Path::new(&out), time, args.iter().any(|a| a == "--stats")).expect("no se pudo guardar el PNG");
        return;
    }
    if let Some(out) = arg_value(&args, "--shot-ship") {
        shot::ship(Path::new(&out), time).expect("error al renderizar la nave");
        return;
    }
    // El bucle interactivo se integra cuando termine el agente `nav`.
    println!("galaxy: usa --shot out.png | --shot-ship out.png [--time T] [--stats]");
}
