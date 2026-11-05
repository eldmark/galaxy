//! ÚNICO módulo que importa raylib: ventana, entrada y presentar el framebuffer.
//! Dueño: agente `ui`. Prohibido usar funciones de dibujo 3D, modelos o cámara de raylib.
use crate::raster::Framebuffer;
use crate::ship::Input;

/// Estado de entrada de un cuadro.
#[derive(Default)]
pub struct UiInput {
    pub ship: Input,
    /// Posición del ratón en píxeles del FRAMEBUFFER (ya reescalada desde la ventana).
    pub mouse: (f32, f32),
    pub mouse_clicked: bool,
    /// Teclas de un solo disparo en este cuadro.
    pub toggle_tour: bool,   // T
    pub toggle_help: bool,   // F1
    pub toggle_stats: bool,  // F3
    pub toggle_culling: bool, // F4
    pub toggle_lod: bool,    // F5
    pub screenshot: bool,    // P
    pub quit: bool,
}

pub struct Platform {}

impl Platform {
    pub fn new(_fb_width: usize, _fb_height: usize, _title: &str) -> Self {
        todo!("ui")
    }
    pub fn should_close(&self) -> bool {
        todo!("ui")
    }
    /// Segundos reales del último cuadro.
    pub fn frame_time(&self) -> f64 {
        todo!("ui")
    }
    pub fn poll(&mut self) -> UiInput {
        todo!("ui")
    }
    /// Sube el framebuffer a una textura 2D y lo muestra escalado.
    pub fn present(&mut self, _fb: &Framebuffer) {
        todo!("ui")
    }
}
