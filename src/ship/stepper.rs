//! Paso fijo con acumulador: la simulación avanza en pasos de `dt` constante sin importar los FPS,
//! así la respuesta de la nave es la misma en equipos rápidos y lentos (rúbrica: tiempo, no cuadros).

#[derive(Clone, Debug)]
pub struct FixedStepper {
    pub dt: f64,
    pub max_steps: u32,
    acc: f64,
    steps: u64,
}

impl FixedStepper {
    pub fn new(dt: f64, max_steps: u32) -> Self {
        Self { dt, max_steps, acc: 0.0, steps: 0 }
    }
    /// 120 Hz y tope de 8 pasos por cuadro (evita la "espiral de la muerte").
    pub fn default_120() -> Self {
        Self::new(1.0 / 120.0, 8)
    }
    /// Suma el tiempo real del cuadro y devuelve cuántos pasos fijos hay que ejecutar.
    pub fn advance(&mut self, frame_dt: f64) -> u32 {
        self.acc += frame_dt.clamp(0.0, 1.0);
        let mut n = (self.acc / self.dt + 1e-9).floor() as u32;
        self.acc -= n as f64 * self.dt;
        if self.acc < 0.0 {
            self.acc = 0.0;
        }
        if n > self.max_steps {
            n = self.max_steps;
            self.acc = 0.0; // descarta el atraso: la simulación se ralentiza en vez de congelarse
        }
        self.steps += n as u64;
        n
    }
    /// Tiempo de simulación tras los pasos ya entregados (el paso k arranca en t = k·dt).
    pub fn time(&self) -> f64 {
        self.steps as f64 * self.dt
    }
    pub fn steps(&self) -> u64 {
        self.steps
    }
    /// Fracción del siguiente paso ya acumulada (para interpolar el dibujo).
    pub fn alpha(&self) -> f64 {
        (self.acc / self.dt).clamp(0.0, 1.0)
    }
}
