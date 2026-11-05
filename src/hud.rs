//! HUD dibujado por nosotros directamente en el framebuffer (fuente bitmap propia). Dueño: agente `ui`.
use crate::raster::{Framebuffer, Stats};

#[derive(Clone, Copy)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x as f32 && px < (self.x + self.w) as f32 && py >= self.y as f32 && py < (self.y + self.h) as f32
    }
}

/// Fuente bitmap 5x7 (ASCII imprimible), `scale` enteros. Fondo transparente.
pub fn draw_text(_fb: &mut Framebuffer, _x: i32, _y: i32, _text: &str, _rgb: [u8; 3], _scale: i32) {
    todo!("ui")
}

/// Botón con estados reposo/hover/activo.
pub fn draw_button(_fb: &mut Framebuffer, _rect: Rect, _label: &str, _hover: bool, _active: bool) {
    todo!("ui")
}

/// Datos que muestra el HUD cada cuadro.
pub struct HudState<'a> {
    pub mode: &'a str,
    pub nearest_body: &'a str,
    pub altitude_m: f64,
    pub speed_ms: f64,
    pub tour_active: bool,
    pub tour_status: &'a str,
    pub show_stats: bool,
    pub show_help: bool,
    pub stats: &'a Stats,
    pub fps: f32,
    pub culling_on: bool,
    pub lod_on: bool,
}

/// Dibuja el HUD completo y devuelve el rectángulo del botón del Tour (para detectar clics).
pub fn draw_hud(_fb: &mut Framebuffer, _state: &HudState, _mouse: (f32, f32)) -> Rect {
    todo!("ui")
}
