//! HUD dibujado por nosotros directamente en el framebuffer (fuente bitmap propia). Dueño: agente `ui`.
//! No usa raylib: todo son escrituras (con mezcla alfa y recorte) al buffer RGBA.
use crate::raster::{Framebuffer, Stats};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

// ---------------------------------------------------------------- paleta fría
const COL_TEXT: [u8; 3] = [214, 232, 250];
const COL_DIM: [u8; 3] = [124, 150, 180];
const COL_ACCENT: [u8; 3] = [96, 206, 255];
const COL_GOOD: [u8; 3] = [120, 235, 190];
const COL_WARN: [u8; 3] = [255, 196, 110];
const COL_PANEL: [u8; 3] = [6, 12, 24];
const COL_BORDER: [u8; 3] = [64, 104, 156];
const COL_SHADOW: [u8; 3] = [0, 0, 0];

// ---------------------------------------------------------------- fuente 5x7
/// Glifos 5x7 para ASCII 32..=95 (espacio..`_`); 7 filas, bit 4 = columna izquierda.
/// Las minúsculas se dibujan como mayúsculas; los acentos se mapean a su letra base.
const FONT: [[u8; 7]; 64] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00], // espacio
    [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04], // !
    [0x0A, 0x0A, 0x0A, 0x00, 0x00, 0x00, 0x00], // "
    [0x0A, 0x0A, 0x1F, 0x0A, 0x1F, 0x0A, 0x0A], // #
    [0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04], // $
    [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03], // %
    [0x0C, 0x12, 0x14, 0x08, 0x15, 0x12, 0x0D], // &
    [0x0C, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00], // '
    [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02], // (
    [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08], // )
    [0x00, 0x04, 0x15, 0x0E, 0x15, 0x04, 0x00], // *
    [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00], // +
    [0x00, 0x00, 0x00, 0x00, 0x0C, 0x04, 0x08], // ,
    [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00], // -
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C], // .
    [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x00], // /
    [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E], // 0
    [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E], // 1
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F], // 2
    [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E], // 3
    [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02], // 4
    [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E], // 5
    [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E], // 6
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08], // 7
    [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E], // 8
    [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C], // 9
    [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00], // :
    [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x04, 0x08], // ;
    [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02], // <
    [0x00, 0x00, 0x1F, 0x00, 0x1F, 0x00, 0x00], // =
    [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08], // >
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04], // ?
    [0x0E, 0x11, 0x01, 0x0D, 0x15, 0x15, 0x0E], // @
    [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11], // A
    [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E], // B
    [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E], // C
    [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C], // D
    [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F], // E
    [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10], // F
    [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F], // G
    [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11], // H
    [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E], // I
    [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C], // J
    [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11], // K
    [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F], // L
    [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11], // M
    [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11], // N
    [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], // O
    [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10], // P
    [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D], // Q
    [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11], // R
    [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E], // S
    [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04], // T
    [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E], // U
    [0x11, 0x11, 0x11, 0x11, 0x11, 0x0A, 0x04], // V
    [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0A], // W
    [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11], // X
    [0x11, 0x11, 0x11, 0x0A, 0x04, 0x04, 0x04], // Y
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F], // Z
    [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E], // [
    [0x00, 0x10, 0x08, 0x04, 0x02, 0x01, 0x00], // \
    [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E], // ]
    [0x04, 0x0A, 0x11, 0x00, 0x00, 0x00, 0x00], // ^
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1F], // _
];

/// Glifo de un carácter cualquiera (minúsculas a mayúsculas, acentos a letra base, resto a `?`).
fn glyph(c: char) -> &'static [u8; 7] {
    let c = match c {
        'á' | 'Á' | 'à' | 'À' | 'ä' | 'Ä' => 'A',
        'é' | 'É' | 'è' | 'È' | 'ë' | 'Ë' => 'E',
        'í' | 'Í' | 'ì' | 'Ì' | 'ï' | 'Ï' => 'I',
        'ó' | 'Ó' | 'ò' | 'Ò' | 'ö' | 'Ö' => 'O',
        'ú' | 'Ú' | 'ù' | 'Ù' | 'ü' | 'Ü' => 'U',
        'ñ' | 'Ñ' => 'N',
        '¿' => '?',
        '¡' => '!',
        '`' | '{' | '}' | '|' | '~' => '?',
        other => other,
    };
    let c = c.to_ascii_uppercase();
    let code = c as u32;
    if (32..=95).contains(&code) { &FONT[(code - 32) as usize] } else { &FONT[('?' as u32 - 32) as usize] }
}

fn clamp_scale(scale: i32) -> i32 {
    scale.clamp(1, 16)
}

/// Ancho en píxeles de `text` (línea más larga) con la escala dada.
pub fn text_width(text: &str, scale: i32) -> i32 {
    let s = clamp_scale(scale);
    let n = text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0) as i32;
    if n == 0 { 0 } else { (n * 6 - 1) * s }
}

/// Alto en píxeles de `text` (todas las líneas).
pub fn text_height(text: &str, scale: i32) -> i32 {
    let s = clamp_scale(scale);
    let lines = text.split('\n').count() as i32;
    (lines * 9 - 2) * s
}

// ---------------------------------------------------------------- primitivas con recorte
#[inline]
fn blend_px(fb: &mut Framebuffer, x: i32, y: i32, rgb: [u8; 3], alpha: u8) {
    if x < 0 || y < 0 || x as usize >= fb.width || y as usize >= fb.height {
        return;
    }
    let i = (y as usize * fb.width + x as usize) * 4;
    if i + 3 >= fb.color.len() {
        return;
    }
    if alpha == 255 {
        fb.color[i] = rgb[0];
        fb.color[i + 1] = rgb[1];
        fb.color[i + 2] = rgb[2];
    } else {
        let a = alpha as u32;
        for k in 0..3 {
            fb.color[i + k] = ((rgb[k] as u32 * a + fb.color[i + k] as u32 * (255 - a)) / 255) as u8;
        }
    }
    fb.color[i + 3] = 255;
}

/// Rectángulo relleno con alfa, recortado al framebuffer.
pub fn fill_rect(fb: &mut Framebuffer, r: Rect, rgb: [u8; 3], alpha: u8) {
    let x0 = r.x.max(0);
    let y0 = r.y.max(0);
    let x1 = r.x.saturating_add(r.w).min(fb.width as i32);
    let y1 = r.y.saturating_add(r.h).min(fb.height as i32);
    for y in y0..y1 {
        for x in x0..x1 {
            blend_px(fb, x, y, rgb, alpha);
        }
    }
}

/// Borde de 1 px.
pub fn stroke_rect(fb: &mut Framebuffer, r: Rect, rgb: [u8; 3], alpha: u8) {
    if r.w <= 0 || r.h <= 0 {
        return;
    }
    fill_rect(fb, Rect { x: r.x, y: r.y, w: r.w, h: 1 }, rgb, alpha);
    fill_rect(fb, Rect { x: r.x, y: r.y + r.h - 1, w: r.w, h: 1 }, rgb, alpha);
    fill_rect(fb, Rect { x: r.x, y: r.y + 1, w: 1, h: r.h - 2 }, rgb, alpha);
    fill_rect(fb, Rect { x: r.x + r.w - 1, y: r.y + 1, w: 1, h: r.h - 2 }, rgb, alpha);
}

/// Fuente bitmap 5x7 (ASCII imprimible), `scale` enteros. Fondo transparente. `\n` baja de línea.
pub fn draw_text(fb: &mut Framebuffer, x: i32, y: i32, text: &str, rgb: [u8; 3], scale: i32) {
    let s = clamp_scale(scale);
    let mut cy = y;
    for line in text.split('\n') {
        let mut cx = x;
        for ch in line.chars() {
            if cx > fb.width as i32 {
                break;
            }
            if cx.saturating_add(5 * s) >= 0 && cy < fb.height as i32 && cy.saturating_add(7 * s) >= 0 {
                let g = glyph(ch);
                for (row, bits) in g.iter().enumerate() {
                    for col in 0..5 {
                        if bits & (0x10 >> col) != 0 {
                            let px = cx.saturating_add(col * s);
                            let py = cy.saturating_add(row as i32 * s);
                            fill_rect(fb, Rect { x: px, y: py, w: s, h: s }, rgb, 255);
                        }
                    }
                }
            }
            cx = cx.saturating_add(6 * s);
        }
        cy = cy.saturating_add(9 * s);
    }
}

/// Texto con sombra negra de 1 px abajo a la derecha.
pub fn draw_text_shadow(fb: &mut Framebuffer, x: i32, y: i32, text: &str, rgb: [u8; 3], scale: i32) {
    draw_text(fb, x.saturating_add(1), y.saturating_add(1), text, COL_SHADOW, scale);
    draw_text(fb, x, y, text, rgb, scale);
}

/// Botón con estados reposo/hover/activo (relleno semitransparente, borde y texto centrado).
pub fn draw_button(fb: &mut Framebuffer, rect: Rect, label: &str, hover: bool, active: bool) {
    let (fill, fa, border, text) = if active {
        ([20, 90, 130], 215, COL_ACCENT, [255, 255, 255])
    } else if hover {
        ([30, 62, 104], 205, [140, 205, 255], [255, 255, 255])
    } else {
        (COL_PANEL, 165, COL_BORDER, COL_TEXT)
    };
    fill_rect(fb, rect, fill, fa);
    stroke_rect(fb, rect, border, 255);
    if active {
        // Franja de acento a la izquierda indica "encendido".
        fill_rect(fb, Rect { x: rect.x + 1, y: rect.y + 1, w: 3, h: (rect.h - 2).max(0) }, COL_ACCENT, 255);
    }
    let scale = 2;
    let tx = rect.x + (rect.w - text_width(label, scale)) / 2;
    let ty = rect.y + (rect.h - 7 * scale) / 2;
    draw_text_shadow(fb, tx, ty, label, text, scale);
}

// ---------------------------------------------------------------- HUD
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
    /// AÑADIDO: el aterrizaje automático está en marcha (el botón pasa a "CANCELAR ATERRIZAJE [L]").
    pub land_active: bool,
}

/// Rectángulos de los botones del HUD (para detectar clics).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HudLayout {
    pub tour: Rect,
    pub land: Rect,
}

/// Formatea una distancia en metros: "850 M", "12.35 KM", "1500 KM".
pub fn format_distance(m: f64) -> String {
    if !m.is_finite() {
        return "--".to_string();
    }
    let a = m.abs();
    if a < 1000.0 {
        format!("{:.0} M", m)
    } else if a < 100_000.0 {
        format!("{:.2} KM", m / 1000.0)
    } else {
        format!("{:.0} KM", m / 1000.0)
    }
}

/// Formatea una velocidad en m/s o km/s.
pub fn format_speed(ms: f64) -> String {
    if !ms.is_finite() {
        return "--".to_string();
    }
    if ms.abs() < 1000.0 { format!("{:.1} M/S", ms) } else { format!("{:.2} KM/S", ms / 1000.0) }
}

fn panel(fb: &mut Framebuffer, r: Rect) {
    fill_rect(fb, r, COL_PANEL, 150);
    stroke_rect(fb, r, COL_BORDER, 200);
}

/// Dibuja el HUD completo y devuelve el rectángulo del botón del Tour (para detectar clics).
pub fn draw_hud(fb: &mut Framebuffer, state: &HudState, mouse: (f32, f32)) -> Rect {
    draw_hud_layout(fb, state, mouse).tour
}

/// Como `draw_hud` pero devuelve también el botón de aterrizaje automático.
pub fn draw_hud_layout(fb: &mut Framebuffer, state: &HudState, mouse: (f32, f32)) -> HudLayout {
    let w = fb.width as i32;
    let h = fb.height as i32;

    // Panel de telemetría (arriba a la izquierda).
    let lines = [
        ("MODO", state.mode.to_string(), COL_ACCENT),
        ("CERCA", state.nearest_body.to_string(), COL_TEXT),
        ("ALT", format_distance(state.altitude_m), COL_TEXT),
        ("VEL", format_speed(state.speed_ms), COL_TEXT),
    ];
    let pw = 232;
    let ph = 10 + lines.len() as i32 * 18;
    panel(fb, Rect { x: 8, y: 8, w: pw, h: ph });
    for (i, (k, v, c)) in lines.iter().enumerate() {
        let y = 14 + i as i32 * 18;
        draw_text_shadow(fb, 16, y, k, COL_DIM, 2);
        draw_text_shadow(fb, 16 + 80, y, v, *c, 2);
    }

    // FPS (arriba a la derecha) y estado de culling/LOD.
    let fps = format!("{:.0} FPS", state.fps);
    let fw = text_width(&fps, 2);
    draw_text_shadow(fb, w - fw - 10, 10, &fps, if state.fps >= 30.0 { COL_GOOD } else { COL_WARN }, 2);
    let flags = format!(
        "CULLING {} [F4]   LOD {} [F5]",
        if state.culling_on { "ON" } else { "OFF" },
        if state.lod_on { "ON" } else { "OFF" }
    );
    let flw = text_width(&flags, 1);
    draw_text_shadow(fb, w - flw - 10, 30, &flags, COL_DIM, 1);
    let hint = "F1 AYUDA  F3 STATS";
    draw_text_shadow(fb, w - text_width(hint, 1) - 10, 42, hint, COL_DIM, 1);

    // Botones (abajo, centrados).
    let bw = text_width("CANCELAR ATERRIZAJE [L]", 2) + 24;
    let bh = 28;
    let gap = 12;
    let total = bw * 2 + gap;
    let by = h - bh - 12;
    let bx = (w - total) / 2;
    let tour = Rect { x: bx, y: by, w: bw, h: bh };
    let land = Rect { x: bx + bw + gap, y: by, w: bw, h: bh };
    let tl = if state.tour_active { "DETENER TOUR [T]" } else { "TOUR AUTOMATICO [T]" };
    let ll = if state.land_active { "CANCELAR ATERRIZAJE [L]" } else { "ATERRIZAR AUTO [L]" };
    draw_button(fb, tour, tl, tour.contains(mouse.0, mouse.1), state.tour_active);
    draw_button(fb, land, ll, land.contains(mouse.0, mouse.1), state.land_active);

    if state.tour_active || state.land_active {
        let st = if state.tour_status.is_empty() { "EN CURSO" } else { state.tour_status };
        let tw = text_width(st, 2);
        let r = Rect { x: (w - tw) / 2 - 10, y: by - 28, w: tw + 20, h: 22 };
        panel(fb, r);
        draw_text_shadow(fb, r.x + 10, r.y + 4, st, COL_GOOD, 2);
    }

    if state.show_stats {
        draw_stats_panel(fb, state);
    }
    if state.show_help {
        draw_help(fb);
    }
    HudLayout { tour, land }
}

fn draw_stats_panel(fb: &mut Framebuffer, state: &HudState) {
    let s = state.stats;
    let rows: Vec<(String, [u8; 3])> = vec![
        ("ESTADISTICAS [F3]".to_string(), COL_ACCENT),
        (format!("DRAW CALLS {} (CULL {})", s.draw_calls_total, s.draw_calls_culled), COL_TEXT),
        (format!("TRI ENTRADA   {}", s.triangles_in), COL_TEXT),
        (format!("TRI BACKFACE  {}", s.triangles_backface), COL_TEXT),
        (format!("TRI CLIPPED   {}", s.triangles_clipped_out), COL_TEXT),
        (format!("TRI DIBUJADOS {}", s.triangles_drawn), COL_TEXT),
        (format!("PIXELES SOMBR. {}", s.pixels_shaded), COL_TEXT),
        (format!("MS VERTICES {:.2}", s.ms_vertex), COL_DIM),
        (format!("MS RASTER   {:.2}", s.ms_raster), COL_DIM),
        (format!("MS SOMBREADO {:.2}", s.ms_shade), COL_DIM),
        (
            format!("CULLING {}  LOD {}", if state.culling_on { "ON" } else { "OFF" }, if state.lod_on { "ON" } else { "OFF" }),
            COL_GOOD,
        ),
    ];
    let w = fb.width as i32;
    let pw = 280;
    let ph = 12 + rows.len() as i32 * 14;
    let r = Rect { x: w - pw - 8, y: 60, w: pw, h: ph };
    panel(fb, r);
    for (i, (t, c)) in rows.iter().enumerate() {
        draw_text_shadow(fb, r.x + 8, r.y + 8 + i as i32 * 14, t, *c, 1);
    }
}

/// Tabla de controles mostrada en la pantalla de ayuda.
pub const HELP_ROWS: [(&str, &str); 17] = [
    ("W / S", "Adelante / atras"),
    ("A / D", "Lateral izquierda / derecha"),
    ("Q / E", "Abajo / arriba"),
    ("FLECHAS", "Cabeceo y guinada"),
    ("Z / C", "Alabeo"),
    ("RATON DER.", "Arrastrar: mirar libremente"),
    ("SHIFT", "Boost"),
    ("CTRL", "Freno"),
    ("G", "Gravedad on/off"),
    ("X", "Asistencia de vuelo on/off"),
    ("T", "Tour automatico"),
    ("L", "Aterrizar automatico"),
    ("1 - 6", "Elegir destino"),
    ("F1", "Esta ayuda"),
    ("F3 / F4 / F5", "Stats / culling / LOD"),
    ("P", "Captura de pantalla"),
    ("ESC", "Salir"),
];

fn draw_help(fb: &mut Framebuffer) {
    let w = fb.width as i32;
    let h = fb.height as i32;
    fill_rect(fb, Rect { x: 0, y: 0, w, h }, [2, 5, 12], 190);
    let pw = 520.min(w - 16);
    let ph = 56 + HELP_ROWS.len() as i32 * 18 + 24;
    let r = Rect { x: (w - pw) / 2, y: ((h - ph) / 2).max(4), w: pw, h: ph };
    panel(fb, r);
    draw_text_shadow(fb, r.x + 16, r.y + 14, "CONTROLES", COL_ACCENT, 3);
    for (i, (k, d)) in HELP_ROWS.iter().enumerate() {
        let y = r.y + 56 + i as i32 * 18;
        draw_text_shadow(fb, r.x + 16, y, k, COL_ACCENT, 2);
        draw_text_shadow(fb, r.x + 16 + 13 * 12, y, d, COL_TEXT, 2);
    }
    draw_text_shadow(fb, r.x + 16, r.y + r.h - 22, "F1 PARA CERRAR", COL_DIM, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit_bbox(fb: &Framebuffer, bg: u8) -> Option<(usize, usize, usize, usize)> {
        let mut bb: Option<(usize, usize, usize, usize)> = None;
        for y in 0..fb.height {
            for x in 0..fb.width {
                if fb.color[(y * fb.width + x) * 4] != bg {
                    bb = Some(match bb {
                        None => (x, y, x, y),
                        Some((a, b, c, d)) => (a.min(x), b.min(y), c.max(x), d.max(y)),
                    });
                }
            }
        }
        bb
    }

    fn state<'a>(stats: &'a Stats) -> HudState<'a> {
        HudState {
            mode: "CRUCERO",
            nearest_body: "VERDALIA",
            altitude_m: 12_345.0,
            speed_ms: 1500.0,
            tour_active: false,
            tour_status: "",
            show_stats: true,
            show_help: false,
            stats,
            fps: 60.0,
            culling_on: true,
            lod_on: false,
            land_active: false,
        }
    }

    #[test]
    fn rect_contains() {
        let r = Rect { x: 10, y: 20, w: 30, h: 40 };
        assert!(r.contains(10.0, 20.0));
        assert!(r.contains(39.9, 59.9));
        assert!(!r.contains(40.0, 30.0));
        assert!(!r.contains(9.9, 30.0));
        assert!(!r.contains(20.0, 60.0));
    }

    #[test]
    fn text_draws_inside_expected_rect() {
        let mut fb = Framebuffer::new(200, 60);
        fb.clear([0, 0, 0]);
        draw_text(&mut fb, 10, 8, "HOLA 123", [255, 255, 255], 2);
        let (x0, y0, x1, y1) = lit_bbox(&fb, 0).expect("debe dibujar");
        assert!(x0 >= 10 && y0 >= 8);
        assert!((x1 as i32) < 10 + text_width("HOLA 123", 2));
        assert!((y1 as i32) < 8 + text_height("HOLA 123", 2));
    }

    #[test]
    fn text_out_of_bounds_does_not_panic() {
        let mut fb = Framebuffer::new(50, 20);
        fb.clear([0, 0, 0]);
        for &(x, y) in &[(-1000, -1000), (-5, -5), (45, 15), (i32::MAX, 0), (0, i32::MAX), (i32::MIN, i32::MIN), (1000, 1000)] {
            draw_text(&mut fb, x, y, "Texto Ñandú\nsegunda línea ~{}", [1, 2, 3], 3);
            draw_text_shadow(&mut fb, x, y, "x", [1, 2, 3], 100);
        }
        draw_text(&mut fb, 0, 0, "", [1, 2, 3], 0);
        draw_text(&mut fb, 0, 0, "A", [1, 2, 3], -4);
    }

    #[test]
    fn hud_on_tiny_framebuffers_does_not_panic() {
        let stats = Stats::default();
        for &(w, h) in &[(0usize, 0usize), (1, 1), (7, 3), (50, 30), (320, 200)] {
            let mut fb = Framebuffer::new(w, h);
            let mut st = state(&stats);
            draw_hud(&mut fb, &st, (-5.0, 1e9));
            st.show_help = true;
            st.tour_active = true;
            st.land_active = true;
            st.altitude_m = f64::NAN;
            draw_hud_layout(&mut fb, &st, (3.0, 3.0));
            draw_button(&mut fb, Rect { x: -50, y: -50, w: 5000, h: 5000 }, "X", true, true);
        }
    }

    #[test]
    fn hud_layout_buttons_inside_960x540_and_distinct() {
        let stats = Stats::default();
        let mut fb = Framebuffer::new(960, 540);
        let l = draw_hud_layout(&mut fb, &state(&stats), (0.0, 0.0));
        for r in [l.tour, l.land] {
            assert!(r.x >= 0 && r.y >= 0 && r.x + r.w <= 960 && r.y + r.h <= 540);
        }
        assert!(l.tour.x + l.tour.w <= l.land.x);
        assert_eq!(draw_hud(&mut fb, &state(&stats), (0.0, 0.0)), l.tour);
    }

    #[test]
    fn formats() {
        assert_eq!(format_distance(850.0), "850 M");
        assert_eq!(format_distance(12_345.0), "12.35 KM");
        assert_eq!(format_distance(1_500_000.0), "1500 KM");
        assert_eq!(format_speed(12.0), "12.0 M/S");
        assert_eq!(format_speed(2000.0), "2.00 KM/S");
    }

    // PNG mínimo sin dependencias (zlib con bloques "stored").
    fn crc32(data: &[u8]) -> u32 {
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c ^= b as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
        }
        !c
    }
    fn write_png(path: &str, fb: &Framebuffer) {
        let mut raw = Vec::new();
        for y in 0..fb.height {
            raw.push(0);
            for x in 0..fb.width {
                let i = (y * fb.width + x) * 4;
                raw.extend_from_slice(&fb.color[i..i + 3]);
            }
        }
        let mut z = vec![0x78, 0x01];
        let mut chunks = raw.chunks(65535).peekable();
        while let Some(c) = chunks.next() {
            z.push(if chunks.peek().is_none() { 1 } else { 0 });
            z.extend_from_slice(&(c.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(c.len() as u16)).to_le_bytes());
            z.extend_from_slice(c);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &v in &raw {
            a = (a + v as u32) % 65521;
            b = (b + a) % 65521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut chunk = |ty: &[u8; 4], data: &[u8]| {
            out.extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut body = ty.to_vec();
            body.extend_from_slice(data);
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc32(&body).to_be_bytes());
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(fb.width as u32).to_be_bytes());
        ihdr.extend_from_slice(&(fb.height as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        chunk(b"IDAT", &z);
        chunk(b"IEND", &[]);
        std::fs::write(path, out).unwrap();
    }

    #[test]
    fn render_reference_pngs() {
        let dir = "/tmp/claude-1000/-home-dmark123-Documents-homework-graficas-galaxy/146d9493-3f05-4f8b-853c-4441b3071a9a/scratchpad";
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let stats = Stats {
            draw_calls_total: 14,
            draw_calls_culled: 5,
            triangles_in: 120_000,
            triangles_backface: 50_000,
            triangles_clipped_out: 300,
            triangles_drawn: 69_700,
            pixels_shaded: 2_100_000,
            ms_vertex: 1.2,
            ms_raster: 3.4,
            ms_shade: 8.8,
            ..Default::default()
        };
        let mut fb = Framebuffer::new(960, 540);
        for y in 0..540 {
            for x in 0..960 {
                let i = (y * 960 + x) * 4;
                fb.color[i..i + 4].copy_from_slice(&[(x / 6) as u8 / 2 + 10, 20 + (y / 8) as u8, 70, 255]);
            }
        }
        let mut st = state(&stats);
        st.tour_active = true;
        st.tour_status = "TRANSITO A SIROCO  ETA 0:42";
        draw_hud_layout(&mut fb, &st, (480.0, 515.0));
        write_png(&format!("{dir}/hud_main.png"), &fb);
        st.show_help = true;
        draw_hud_layout(&mut fb, &st, (0.0, 0.0));
        write_png(&format!("{dir}/hud_help.png"), &fb);
    }
}
