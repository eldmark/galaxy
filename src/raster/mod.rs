//! Pipeline de rasterizado por software. Dueño: agente `raster`.
mod pipeline;
#[cfg(test)]
mod tests;

use crate::geometry::Mesh;
use crate::math::{Mat4, Vec3};

/// Buffer de color RGBA (u8) y profundidad.
/// `depth` guarda 1/w (mayor = más cerca, 0.0 = infinito) para que la interpolación sea lineal en pantalla.
pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    pub color: Vec<u8>,
    pub depth: Vec<f32>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            color: vec![0; width * height * 4],
            depth: vec![0.0; width * height],
        }
    }
    pub fn clear(&mut self, rgb: [u8; 3]) {
        for px in self.color.chunks_exact_mut(4) {
            px.copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        self.depth.fill(0.0);
    }
    /// Guarda el color actual como PNG.
    pub fn save_png(&self, path: &std::path::Path) -> std::io::Result<()> {
        crate::png::save(path, self.width, self.height, &self.color)
    }
}

/// Datos interpolados que recibe un shader de fragmentos.
pub struct FragIn {
    /// Posición interpolada en espacio del objeto (para ruido 3D estable en la superficie).
    pub p_local: Vec3,
    /// Normal interpolada, en espacio de mundo, normalizada.
    pub normal: Vec3,
    pub uv: [f32; 2],
    /// Color de vértice interpolado, 0..1.
    pub color: [f32; 3],
    /// Dirección unitaria hacia la cámara, en espacio de mundo.
    pub view_dir: Vec3,
    /// Distancia cámara-fragmento (para `detail`/LOD de shader).
    pub distance: f32,
}

/// Datos constantes por llamada de dibujo.
#[derive(Clone, Copy)]
pub struct Uniforms {
    /// Dirección unitaria HACIA la luz (el sol), en espacio de mundo.
    pub light_dir: Vec3,
    /// Tiempo de simulación en segundos (no depende de los cuadros).
    pub time: f32,
    /// 1.0 = detalle completo; menor reduce octavas de ruido en shaders caros.
    pub detail: f32,
    /// Matriz de modelo, por si el shader necesita pasar de local a mundo.
    pub model: Mat4,
}

/// Shader de fragmentos propio. Devuelve RGB en 0.0..=1.0.
pub trait FragmentShader: Sync + Send {
    fn shade(&self, f: &FragIn, u: &Uniforms) -> [f32; 3];
}

pub struct DrawCall<'a> {
    pub mesh: &'a Mesh,
    pub model: Mat4,
    pub shader: &'a dyn FragmentShader,
    pub uniforms: Uniforms,
}

pub struct RenderOptions {
    pub frustum_culling: bool,
    pub backface_culling: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self { frustum_culling: true, backface_culling: true }
    }
}

/// Contadores por cuadro (se muestran en el HUD; sirven para comparar culling/LOD).
#[derive(Default, Clone, Debug)]
pub struct Stats {
    pub draw_calls_total: u32,
    pub draw_calls_culled: u32,
    pub triangles_in: u64,
    pub triangles_backface: u64,
    pub triangles_clipped_out: u64,
    pub triangles_drawn: u64,
    pub pixels_shaded: u64,
    pub ms_vertex: f32,
    pub ms_raster: f32,
    pub ms_shade: f32,
    /// Fondo estelar (lo mide el coordinador, no el rasterizador).
    pub ms_sky: f32,
}

/// Dibuja todas las llamadas con `view` y `proj`. No limpia el framebuffer.
/// Los contadores de `stats` se ACUMULAN (el llamador pone `Stats::default()` al inicio del cuadro).
/// Las mallas con `bounds_radius <= 0` no se someten a frustum culling.
pub fn render_scene(
    fb: &mut Framebuffer,
    view: Mat4,
    proj: Mat4,
    calls: &[DrawCall],
    opts: &RenderOptions,
    stats: &mut Stats,
) {
    pipeline::render(fb, view, proj, calls, opts, stats);
}
