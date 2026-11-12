//! ÚNICO módulo que importa raylib: ventana, entrada, audio y presentación del framebuffer.
//! Raylib crea la ventana, lee teclado y ratón, reproduce WAV y muestra una textura 2D.
//! El dibujo 3D, los modelos y las cámaras se calculan fuera de raylib.
use crate::math::Vec3;
use crate::raster::Framebuffer;
use crate::ship::Input;
use raylib::prelude::*;
use raylib::core::audio::{Music, RaylibAudio, Sound};

/// Audio 2D del juego. Raylib sigue concentrado en este módulo de plataforma.
pub struct AudioDevice(RaylibAudio);
pub struct GameAudio<'a> {
    menu: Option<Music<'a>>,
    game: Option<Music<'a>>,
    shot: Option<Sound<'a>>,
    hit: Option<Sound<'a>>,
    pickup: Option<Sound<'a>>,
    victory: Option<Sound<'a>>,
}
impl AudioDevice {
    pub fn init() -> Option<Self> { RaylibAudio::init_audio_device().ok().map(Self) }
    pub fn load(&self) -> GameAudio<'_> {
        let menu = self.0.new_music("assets/audio/main_theme.wav").ok();
        let game = self.0.new_music("assets/audio/asault_theme.wav").ok();
        let shot = self.0.new_sound("assets/audio/shot.wav").ok();
        let hit = self.0.new_sound("assets/audio/damage_static.wav").ok();
        let pickup = self.0.new_sound("assets/audio/pickup_item.wav").ok();
        let victory = self.0.new_sound("assets/audio/victory_endgame.wav").ok();
        if let Some(s) = &menu { s.set_volume(0.13); }
        if let Some(s) = &game { s.set_volume(0.13); }
        if let Some(s) = &shot { s.set_volume(0.38); }
        if let Some(s) = &hit { s.set_volume(0.48); }
        if let Some(s) = &pickup { s.set_volume(0.65); }
        if let Some(s) = &victory { s.set_volume(0.75); }
        GameAudio { menu, game, shot, hit, pickup, victory }
    }
}
impl GameAudio<'_> {
    pub fn menu(&self) { if let Some(s) = &self.game { s.stop_stream(); } if let Some(s) = &self.menu { s.play_stream(); } }
    pub fn game(&self) { if let Some(s) = &self.menu { s.stop_stream(); } if let Some(s) = &self.game { s.play_stream(); } }
    pub fn update(&self) { if let Some(s) = &self.menu { if s.is_stream_playing() { s.update_stream(); } } if let Some(s) = &self.game { if s.is_stream_playing() { s.update_stream(); } } }
    pub fn shot(&self) { if let Some(s) = &self.shot { s.play(); } }
    pub fn hit(&self) { if let Some(s) = &self.hit { s.play(); } }
    pub fn pickup(&self) { if let Some(s) = &self.pickup { s.play(); } }
    pub fn victory(&self) { if let Some(m) = &self.game { m.stop_stream(); } if let Some(s) = &self.victory { s.play(); } }
}

/// Estado de entrada de un cuadro.
#[derive(Default)]
pub struct UiInput {
    pub ship: Input,
    /// Posición del ratón en píxeles del FRAMEBUFFER (ya reescalada desde la ventana).
    pub mouse: (f32, f32),
    pub mouse_clicked: bool,
    /// Teclas de un solo disparo en este cuadro.
    pub toggle_help: bool,   // F1
    pub toggle_stats: bool,  // F3
    pub toggle_culling: bool, // F4
    pub toggle_lod: bool,    // F5
    pub toggle_resolution: bool, // F6
    pub start_game: bool, // Enter o espacio en el menú
    pub toggle_board: bool, // B: aterrizar y salir / abordar
    pub collect: bool, // E: recoger tesoro a pie
    pub fire: bool, // Espacio mantenido: disparar
    pub toggle_normalmap: bool, // N
    pub screenshot: bool,    // P
    pub quit: bool,
    /// AÑADIDO. G: gravedad on/off.
    pub toggle_gravity: bool,
    /// AÑADIDO. X: asistencia de vuelo on/off.
    pub toggle_assist: bool,
    /// AÑADIDO. L: aterrizar automático.
    pub land_auto: bool,
    /// AÑADIDO. O: mostrar/ocultar las líneas de órbita.
    pub toggle_orbits: bool,
    /// AÑADIDO. K o Supr: reaparecer manualmente.
    pub respawn: bool,
    /// AÑADIDO. Teclas 1..6 -> Some(0..=5).
    pub select_body: Option<usize>,
    /// AÑADIDO. Delta de ratón (píxeles de ventana) acumulado en el cuadro mientras el botón derecho está pulsado
    /// (mirada libre de la cámara; NO gira la nave).
    pub mouse_look: (f32, f32),
    /// AÑADIDO. Botón derecho pulsado: mirada libre (`CameraRig::update(.., free_look)`).
    pub free_look: bool,
    /// AÑADIDO. Modo "el ratón timonea la nave" activo (se alterna con M o el botón central).
    /// El delta ya va incluido en `ship.rotate` (via `map_keys`).
    pub mouse_steer: bool,
    /// AÑADIDO. Rueda del ratón (positivo = hacia delante).
    pub scroll: f32,
}

// ------------------------------------------------------------------ lógica pura (sin raylib)

/// Teclas mantenidas pulsadas en este cuadro (independiente de raylib para poder probarlo).
#[derive(Default, Clone, Copy, Debug)]
pub struct KeyState {
    pub w: bool,
    pub s: bool,
    pub a: bool,
    pub d: bool,
    pub q: bool,
    pub e: bool,
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub z: bool,
    pub c: bool,
    pub shift: bool,
    pub ctrl: bool,
    /// Botón derecho pulsado (mirada libre de la cámara; no gira la nave).
    pub right_mouse: bool,
    /// Modo "ratón timonea" activo (M / botón central): el delta del ratón gira la nave.
    pub mouse_steer: bool,
    /// Delta del ratón en píxeles de ventana.
    pub mouse_delta: (f32, f32),
}

fn axis(pos: bool, neg: bool) -> f32 {
    (pos as i32 - neg as i32) as f32
}

/// Píxeles de arrastre que equivalen a giro máximo (1.0) en `rotate`.
pub const MOUSE_FULL_TURN_PX: f32 = 12.0;

/// Mapea teclas a `ship::Input`. Opuestos se cancelan; todos los valores quedan en [-1, 1].
/// Convención: thrust.z +1 = adelante (W), thrust.x +1 = derecha (D), thrust.y +1 = arriba (E).
/// El botón derecho NO gira la nave (mirada libre); el ratón solo timonea con `mouse_steer`.
/// Convención de `ship::Input` (la que usa la física): rotate.x +1 = morro arriba (flecha arriba / ratón arriba),
/// rotate.y +1 = guiñada a la DERECHA (flecha derecha / ratón a la derecha), rotate.z +1 = alabeo a la DERECHA (C).
pub fn map_keys(k: &KeyState) -> Input {
    let thrust = Vec3::new(axis(k.d, k.a), axis(k.e, k.q), axis(k.w, k.s));
    let mut pitch = axis(k.up, k.down);
    let mut yaw = axis(k.right, k.left);
    let roll = axis(k.c, k.z);
    // Solo el modo de timoneo mueve la nave con el ratón; el botón derecho es mirada libre (cámara).
    if k.mouse_steer && !k.right_mouse {
        pitch += -k.mouse_delta.1 / MOUSE_FULL_TURN_PX;
        yaw += k.mouse_delta.0 / MOUSE_FULL_TURN_PX;
    }
    let cl = |v: f32| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 };
    Input { thrust, rotate: Vec3::new(cl(pitch), cl(yaw), cl(roll)), boost: k.shift, brake: k.ctrl }
}

/// Rectángulo (x, y, w, h) en píxeles de ventana donde se dibuja el framebuffer
/// manteniendo la relación de aspecto (barras negras / letterbox).
pub fn letterbox(win_w: f32, win_h: f32, fb_w: f32, fb_h: f32) -> (f32, f32, f32, f32) {
    if win_w <= 0.0 || win_h <= 0.0 || fb_w <= 0.0 || fb_h <= 0.0 {
        return (0.0, 0.0, win_w.max(0.0), win_h.max(0.0));
    }
    let scale = (win_w / fb_w).min(win_h / fb_h);
    let w = fb_w * scale;
    let h = fb_h * scale;
    ((win_w - w) * 0.5, (win_h - h) * 0.5, w, h)
}

/// Convierte una posición de ventana a coordenadas del framebuffer (puede salir de [0, fb) en las barras).
pub fn window_to_fb(pos: (f32, f32), win: (f32, f32), fb: (f32, f32)) -> (f32, f32) {
    let (x, y, w, h) = letterbox(win.0, win.1, fb.0, fb.1);
    if w <= 0.0 || h <= 0.0 {
        return (0.0, 0.0);
    }
    ((pos.0 - x) / w * fb.0, (pos.1 - y) / h * fb.1)
}

// ------------------------------------------------------------------ ventana raylib

pub struct Platform {
    // Orden de campos = orden de drop: la textura debe liberarse antes de cerrar la ventana.
    texture: Texture2D,
    rl: RaylibHandle,
    thread: RaylibThread,
    fb_w: usize,
    fb_h: usize,
    frame_time: f64,
    /// Modo "ratón timonea" (alterna M / botón central); mientras está activo el cursor se captura.
    mouse_steer: bool,
}

impl Platform {
    pub fn new(fb_width: usize, fb_height: usize, title: &str) -> Self {
        let (mut rl, thread) = raylib::init()
            .size(fb_width as i32, fb_height as i32)
            .title(title)
            .resizable()
            .build();
        rl.set_target_fps(60);
        rl.set_exit_key(None); // Esc lo gestionamos nosotros (UiInput::quit)
        rl.set_window_min_size(320, 180);
        let image = Image::gen_image_color(fb_width as i32, fb_height as i32, Color::BLACK);
        let texture = rl.load_texture_from_image(&thread, &image).expect("no se pudo crear la textura");
        Self { rl, thread, texture, fb_w: fb_width, fb_h: fb_height, frame_time: 1.0 / 60.0, mouse_steer: false }
    }

    pub fn should_close(&self) -> bool {
        self.rl.window_should_close()
    }

    /// Segundos reales del último cuadro.
    pub fn frame_time(&self) -> f64 {
        self.frame_time
    }

    pub fn poll(&mut self) -> UiInput {
        use KeyboardKey::*;
        self.frame_time = self.rl.get_frame_time() as f64;
        if self.rl.is_key_pressed(KEY_M) || self.rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_MIDDLE) {
            self.mouse_steer = !self.mouse_steer;
            // captura el cursor para que el delta no se agote en el borde de la ventana
            if self.mouse_steer {
                self.rl.disable_cursor();
            } else {
                self.rl.enable_cursor();
            }
        }
        let rl = &self.rl;
        let mouse_steer = self.mouse_steer;
        let right_mouse = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_RIGHT);
        let md = rl.get_mouse_delta();
        let keys = KeyState {
            w: rl.is_key_down(KEY_W),
            s: rl.is_key_down(KEY_S),
            a: rl.is_key_down(KEY_A),
            d: rl.is_key_down(KEY_D),
            q: rl.is_key_down(KEY_Q),
            e: rl.is_key_down(KEY_E),
            up: rl.is_key_down(KEY_UP),
            down: rl.is_key_down(KEY_DOWN),
            left: rl.is_key_down(KEY_LEFT),
            right: rl.is_key_down(KEY_RIGHT),
            z: rl.is_key_down(KEY_Z),
            c: rl.is_key_down(KEY_C),
            shift: rl.is_key_down(KEY_LEFT_SHIFT) || rl.is_key_down(KEY_RIGHT_SHIFT),
            ctrl: rl.is_key_down(KEY_LEFT_CONTROL) || rl.is_key_down(KEY_RIGHT_CONTROL),
            right_mouse,
            mouse_steer,
            mouse_delta: (md.x, md.y),
        };
        let mp = rl.get_mouse_position();
        let win = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        let digits = [KEY_ONE, KEY_TWO, KEY_THREE, KEY_FOUR, KEY_FIVE, KEY_SIX];
        UiInput {
            ship: map_keys(&keys),
            mouse: window_to_fb((mp.x, mp.y), win, (self.fb_w as f32, self.fb_h as f32)),
            mouse_clicked: rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT),
            toggle_help: rl.is_key_pressed(KEY_F1),
            toggle_stats: rl.is_key_pressed(KEY_F3),
            toggle_culling: rl.is_key_pressed(KEY_F4),
            toggle_lod: rl.is_key_pressed(KEY_F5),
            toggle_resolution: rl.is_key_pressed(KEY_F6),
            start_game: rl.is_key_pressed(KEY_ENTER) || rl.is_key_pressed(KEY_SPACE),
            toggle_board: rl.is_key_pressed(KEY_B),
            collect: rl.is_key_pressed(KEY_E),
            fire: rl.is_key_down(KEY_SPACE),
            toggle_normalmap: rl.is_key_pressed(KEY_N),
            screenshot: rl.is_key_pressed(KEY_P),
            quit: rl.is_key_pressed(KEY_ESCAPE),
            toggle_gravity: rl.is_key_pressed(KEY_G),
            toggle_assist: rl.is_key_pressed(KEY_X),
            land_auto: rl.is_key_pressed(KEY_L),
            toggle_orbits: rl.is_key_pressed(KEY_O),
            respawn: rl.is_key_pressed(KEY_K) || rl.is_key_pressed(KEY_DELETE),
            select_body: digits.iter().position(|&k| rl.is_key_pressed(k)),
            mouse_look: if right_mouse { (md.x, md.y) } else { (0.0, 0.0) },
            free_look: right_mouse,
            mouse_steer,
            scroll: rl.get_mouse_wheel_move(),
        }
    }

    /// Sube el framebuffer a una textura 2D y lo muestra escalado (mantiene el aspecto).
    pub fn present(&mut self, fb: &Framebuffer) {
        if fb.width != self.fb_w || fb.height != self.fb_h || fb.color.len() != fb.width * fb.height * 4 {
            return;
        }
        let _ = self.texture.update_texture(&fb.color);
        let win_w = self.rl.get_screen_width() as f32;
        let win_h = self.rl.get_screen_height() as f32;
        let (x, y, w, h) = letterbox(win_w, win_h, self.fb_w as f32, self.fb_h as f32);
        let mut d = self.rl.begin_drawing(&self.thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_pro(
            &self.texture,
            Rectangle::new(0.0, 0.0, self.fb_w as f32, self.fb_h as f32),
            Rectangle::new(x, y, w, h),
            Vector2::new(0.0, 0.0),
            0.0,
            Color::WHITE,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opposites_cancel_and_range() {
        let k = KeyState { w: true, s: true, a: true, d: true, q: true, e: true, up: true, down: true, left: true, right: true, z: true, c: true, ..Default::default() };
        let i = map_keys(&k);
        assert_eq!(i.thrust.x, 0.0);
        assert_eq!(i.thrust.y, 0.0);
        assert_eq!(i.thrust.z, 0.0);
        assert_eq!(i.rotate.x, 0.0);
        assert_eq!(i.rotate.y, 0.0);
        assert_eq!(i.rotate.z, 0.0);
    }

    #[test]
    fn single_keys_and_modifiers() {
        let i = map_keys(&KeyState { w: true, d: true, e: true, shift: true, ctrl: true, ..Default::default() });
        assert_eq!((i.thrust.x, i.thrust.y, i.thrust.z), (1.0, 1.0, 1.0));
        assert!(i.boost && i.brake);
        let i = map_keys(&KeyState { s: true, a: true, q: true, ..Default::default() });
        assert_eq!((i.thrust.x, i.thrust.y, i.thrust.z), (-1.0, -1.0, -1.0));
        assert!(!i.boost && !i.brake);
    }

    #[test]
    fn rotation_follows_ship_input_convention() {
        let r = map_keys(&KeyState { right: true, c: true, up: true, ..Default::default() }).rotate;
        assert_eq!((r.x, r.y, r.z), (1.0, 1.0, 1.0));
        let l = map_keys(&KeyState { left: true, z: true, down: true, ..Default::default() }).rotate;
        assert_eq!((l.x, l.y, l.z), (-1.0, -1.0, -1.0));
        let m = map_keys(&KeyState { mouse_steer: true, mouse_delta: (6.0, 0.0), ..Default::default() }).rotate;
        assert!(m.y > 0.0, "con el ratón timoneando, a la derecha gira a la derecha");
        let m = map_keys(&KeyState { mouse_steer: true, mouse_delta: (0.0, -6.0), ..Default::default() }).rotate;
        assert!(m.x > 0.0, "ratón arriba = morro arriba");
    }

    #[test]
    fn right_button_is_free_look_and_never_steers() {
        let r = map_keys(&KeyState { right_mouse: true, mouse_delta: (40.0, -40.0), ..Default::default() }).rotate;
        assert_eq!((r.x, r.y, r.z), (0.0, 0.0, 0.0));
        // aunque el timoneo esté activo, con el botón derecho (mirada libre) la nave no gira
        let r = map_keys(&KeyState { right_mouse: true, mouse_steer: true, mouse_delta: (40.0, 40.0), ..Default::default() }).rotate;
        assert_eq!((r.x, r.y, r.z), (0.0, 0.0, 0.0));
        // las flechas siguen girando mientras se mira libremente
        let r = map_keys(&KeyState { right_mouse: true, right: true, ..Default::default() }).rotate;
        assert_eq!(r.y, 1.0);
    }

    #[test]
    fn mouse_only_when_steering_and_clamped() {
        let none = map_keys(&KeyState { mouse_delta: (50.0, 50.0), ..Default::default() });
        assert_eq!((none.rotate.x, none.rotate.y), (0.0, 0.0));
        let big = map_keys(&KeyState { mouse_steer: true, mouse_delta: (1e6, -1e6), up: true, left: true, ..Default::default() });
        for v in [big.rotate.x, big.rotate.y, big.rotate.z] {
            assert!((-1.0..=1.0).contains(&v));
        }
        let nan = map_keys(&KeyState { mouse_steer: true, mouse_delta: (f32::NAN, f32::INFINITY), ..Default::default() });
        assert_eq!((nan.rotate.x, nan.rotate.y), (0.0, 0.0));
    }

    #[test]
    fn letterbox_same_aspect_fills() {
        assert_eq!(letterbox(1920.0, 1080.0, 960.0, 540.0), (0.0, 0.0, 1920.0, 1080.0));
    }

    #[test]
    fn letterbox_bars() {
        // Ventana cuadrada: barras arriba y abajo.
        let (x, y, w, h) = letterbox(800.0, 800.0, 960.0, 540.0);
        assert_eq!((x, w), (0.0, 800.0));
        assert!((h - 450.0).abs() < 1e-3 && (y - 175.0).abs() < 1e-3);
        // Ventana muy ancha: barras laterales.
        let (x, y, w, h) = letterbox(2000.0, 540.0, 960.0, 540.0);
        assert_eq!((y, h), (0.0, 540.0));
        assert!((w - 960.0).abs() < 1e-3 && (x - 520.0).abs() < 1e-3);
    }

    #[test]
    fn window_to_fb_mapping() {
        let fb = (960.0, 540.0);
        // sin escalado
        assert_eq!(window_to_fb((100.0, 50.0), (960.0, 540.0), fb), (100.0, 50.0));
        // escala x2
        assert_eq!(window_to_fb((200.0, 100.0), (1920.0, 1080.0), fb), (100.0, 50.0));
        // letterbox vertical: esquina sup-izq del framebuffer en (0,175)
        let p = window_to_fb((0.0, 175.0), (800.0, 800.0), fb);
        assert!(p.0.abs() < 1e-3 && p.1.abs() < 1e-3);
        let p = window_to_fb((800.0, 625.0), (800.0, 800.0), fb);
        assert!((p.0 - 960.0).abs() < 1e-2 && (p.1 - 540.0).abs() < 1e-2);
        // en la barra: fuera de rango (negativo)
        assert!(window_to_fb((10.0, 10.0), (800.0, 800.0), fb).1 < 0.0);
        // ventana degenerada
        assert_eq!(window_to_fb((5.0, 5.0), (0.0, 0.0), fb), (0.0, 0.0));
    }

    /// Prueba manual con ventana real: `cargo test smoke_window -- --ignored`.
    #[test]
    #[ignore]
    fn smoke_window() {
        let mut p = Platform::new(960, 540, "galaxy smoke");
        let mut fb = Framebuffer::new(960, 540);
        let stats = crate::raster::Stats::default();
        let terrain = crate::terrain::TerrainStats::default();
        let scatter = crate::scatter::ScatterStats::default();
        for _ in 0..60 {
            let inp = p.poll();
            fb.clear([10, 20, 40]);
            let st = crate::hud::HudState {
                mode: "CRUCERO", nearest_body: "VERDALIA", altitude_m: 1234.0, speed_ms: 5.0, tour_active: false, spectator: false, tour_available: true,
                tour_status: "", show_stats: true, show_help: false, stats: &stats, fps: 60.0,
                terrain: &terrain, scatter: &scatter, scale: 1.0, scale_auto: false,
                culling_on: true, lod_on: true, land_active: false,
            };
            crate::hud::draw_hud_layout(&mut fb, &st, inp.mouse);
            p.present(&fb);
        }
        assert!(p.frame_time() > 0.0);
    }
}
