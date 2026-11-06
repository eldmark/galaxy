//! Tabla de especies por planeta (`Species`): qué modelo, cuántos, dónde y con qué reglas.
//! El algoritmo de distribución (`place.rs`) es el mismo para todos; solo cambian estos parámetros.
use crate::math::Vec3;
use crate::world::BodyKind;

/// Los cuatro modelos propios que se colocan sobre los planetas (rúbrica: 5 pts por tipo, hasta 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelId {
    /// `assets/models/tree.obj`: tronco y copa facetada (Verdalia).
    Tree,
    /// `assets/models/obsidian.obj`: espiras de obsidiana (Fornax).
    Obsidian,
    /// `assets/models/ice_crystal.obj`: racimo de cristales (Nix).
    Ice,
    /// `assets/models/building.obj`: monumento pixelado de cubos (Mutante).
    Building,
}

impl ModelId {
    pub const ALL: [ModelId; 4] = [ModelId::Tree, ModelId::Obsidian, ModelId::Ice, ModelId::Building];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn file(self) -> &'static str {
        match self {
            ModelId::Tree => "tree.obj",
            ModelId::Obsidian => "obsidian.obj",
            ModelId::Ice => "ice_crystal.obj",
            ModelId::Building => "building.obj",
        }
    }
}

/// Una especie: reglas de colocación de un modelo en un planeta.
#[derive(Clone, Debug)]
pub struct Species {
    pub name: &'static str,
    pub model: ModelId,
    /// Separación (m) de la rejilla de candidatos. Cada celda de scatter (un chunk de nivel 8) se divide en
    /// `round(lado / cell_m)^2` candidatos; cada uno puede o no convertirse en instancia.
    pub cell_m: f32,
    /// 0 = candidatos en el centro de su casilla (cuadrícula, estilo pixel); 1 = jitter en toda la casilla.
    pub jitter: f32,
    /// Probabilidad máxima de que un candidato dentro de una mancha se convierta en instancia.
    pub density: f32,
    /// Ruido de agrupamiento: fbm de valor sobre `dir * noise_freq` (ciclos por radio del planeta).
    /// Solo hay instancias donde `fbm > noise_threshold`; la probabilidad sube suavemente hasta
    /// `noise_threshold + noise_soft`. Da bosques en manchas, racimos de cristales, etc.
    pub noise_freq: f32,
    pub noise_threshold: f32,
    pub noise_soft: f32,
    pub noise_offset: Vec3,
    /// Altura del terreno (m sobre el radio nominal, `height * radio`) permitida.
    pub min_alt_m: f32,
    pub max_alt_m: f32,
    /// Pendiente máxima (grados entre la normal del relieve y la vertical local).
    pub max_slope_deg: f32,
    /// Radio de la huella (m): se muestrea el relieve a esa distancia para la normal y la planitud.
    pub foot_m: f32,
    /// Desnivel máximo (m) permitido dentro de la huella (edificios: solo suelo plano).
    pub max_range_m: f32,
    /// Cuánto se entierra la base (m) para que no flote por la interpolación de la malla del suelo.
    pub sink_m: f32,
    /// Rango de escala uniforme (0.7..1.4).
    pub scale: (f32, f32),
    /// 0 = giro aleatorio continuo alrededor de la normal; si no, el giro es múltiplo de este ángulo (rad).
    pub yaw_step: f32,
    /// Distancias de LOD (m, al chunk más cercano): modelo completo < d0, simplificado < d1, nada más allá.
    pub lod_dist: [f32; 2],
}

impl Species {
    fn base(name: &'static str, model: ModelId) -> Self {
        Self {
            name,
            model,
            cell_m: 6.0,
            jitter: 1.0,
            density: 0.6,
            noise_freq: 30.0,
            noise_threshold: 0.5,
            noise_soft: 0.15,
            noise_offset: Vec3::ZERO,
            min_alt_m: f32::NEG_INFINITY,
            max_alt_m: f32::INFINITY,
            max_slope_deg: 30.0,
            foot_m: 1.5,
            max_range_m: 3.0,
            sink_m: 0.25,
            scale: (0.7, 1.4),
            yaw_step: 0.0,
            lod_dist: [60.0, 200.0],
        }
    }
}

/// Especies de cada cuerpo. Siroco y el sol no tienen suelo: lista vacía.
pub fn species_for(kind: BodyKind) -> Vec<Species> {
    match kind {
        BodyKind::Sun | BodyKind::Siroco => Vec::new(),
        // Bosques en manchas (ruido de ~90 m de largo de onda), solo en tierra firme (altura > 0).
        BodyKind::Verdalia => vec![Species {
            cell_m: 6.0,
            density: 0.75,
            noise_freq: 38.0,
            noise_threshold: 0.50,
            noise_soft: 0.12,
            noise_offset: Vec3::new(11.0, 3.0, 5.0),
            min_alt_m: 1.0,
            max_slope_deg: 24.0,
            foot_m: 1.5,
            sink_m: 0.3,
            ..Species::base("arbol", ModelId::Tree)
        }],
        // Espiras de obsidiana sueltas por zonas; admiten más pendiente.
        BodyKind::Fornax => vec![Species {
            cell_m: 9.0,
            density: 0.8,
            noise_freq: 20.0,
            noise_threshold: 0.38,
            noise_soft: 0.15,
            noise_offset: Vec3::new(2.0, 9.0, 4.0),
            max_slope_deg: 32.0,
            foot_m: 1.5,
            sink_m: 0.35,
            ..Species::base("espira", ModelId::Obsidian)
        }],
        // Cristales en grupos: umbral alto del ruido = racimos compactos y densos.
        BodyKind::Nix => vec![Species {
            cell_m: 5.0,
            density: 0.8,
            noise_freq: 46.0,
            noise_threshold: 0.60,
            noise_soft: 0.08,
            noise_offset: Vec3::new(7.0, 1.0, 8.0),
            max_slope_deg: 28.0,
            foot_m: 1.2,
            sink_m: 0.25,
            ..Species::base("cristal", ModelId::Ice)
        }],
        // Edificios sobre una cuadrícula (sin jitter), giros de 90 grados y solo en suelo plano:
        // el relieve de Mutante es escalonado, así que se exige poco desnivel en la huella.
        BodyKind::Mutante => vec![Species {
            cell_m: 12.0,
            jitter: 0.0,
            density: 0.5,
            noise_freq: 14.0,
            noise_threshold: 0.45,
            noise_soft: 0.15,
            noise_offset: Vec3::new(5.0, 6.0, 1.0),
            max_slope_deg: 3.0,
            foot_m: 3.6,
            max_range_m: 0.35,
            sink_m: 0.4,
            scale: (0.8, 1.2),
            yaw_step: std::f32::consts::FRAC_PI_2,
            ..Species::base("edificio", ModelId::Building)
        }],
    }
}
