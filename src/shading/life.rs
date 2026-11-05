//! Vida de Conway / HighLife precalculada. Línea de tiempo discreta de solo lectura.
//!
//! Decisión: el tablero NO se recalcula por píxel ni se muta. Al crear el shader se
//! simulan `STEPS` generaciones y se guardan todas; `sample(.., t)` elige la generación
//! `floor(t / STEP_SECONDS) % STEPS`. Es una función pura del tiempo de simulación
//! (mismo t => mismo resultado), `Sync`, y no necesita `update`. Pasadas `STEPS`
//! generaciones la animación vuelve a empezar (ciclo de STEPS*STEP_SECONDS s).

pub const STEP_SECONDS: f32 = 0.45;
pub const STEPS: usize = 160;

pub struct LifeTimeline {
    width: usize,
    height: usize,
    gens: Vec<Vec<bool>>,
}

impl LifeTimeline {
    pub fn new(width: usize, height: usize, seed: u32, highlife: bool) -> Self {
        let mut state = seed;
        let mut cells = Vec::with_capacity(width * height);
        for _ in 0..width * height {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            cells.push(state as f64 / 4_294_967_296.0 < 0.37);
        }
        let mut gens = Vec::with_capacity(STEPS);
        gens.push(cells);
        for i in 1..STEPS {
            let next = step(&gens[i - 1], width, height, highlife);
            gens.push(next);
        }
        Self { width, height, gens }
    }

    pub fn generation_at(time: f32) -> usize {
        ((time.max(0.0) / STEP_SECONDS).floor() as usize) % STEPS
    }

    /// Celda viva en (u, v) (u envuelve, v se recorta) en la generación del instante `time`.
    pub fn sample(&self, u: f32, v: f32, time: f32) -> bool {
        let x = (u.rem_euclid(1.0) * self.width as f32).floor() as usize % self.width;
        let y = (v.clamp(0.0, 0.999_999) * self.height as f32).floor() as usize;
        self.gens[Self::generation_at(time)][y.min(self.height - 1) * self.width + x]
    }

    #[cfg(test)]
    pub fn gen_cells(&self, i: usize) -> &[bool] {
        &self.gens[i]
    }
}

fn step(cells: &[bool], w: usize, h: usize, highlife: bool) -> Vec<bool> {
    let at = |x: isize, y: isize| {
        cells[y.rem_euclid(h as isize) as usize * w + x.rem_euclid(w as isize) as usize]
    };
    let mut next = vec![false; cells.len()];
    for y in 0..h {
        for x in 0..w {
            let mut n = 0;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if (dx != 0 || dy != 0) && at(x as isize + dx, y as isize + dy) {
                        n += 1;
                    }
                }
            }
            let alive = at(x as isize, y as isize);
            next[y * w + x] = if alive { n == 2 || n == 3 } else { n == 3 || (highlife && n == 6) };
        }
    }
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blinker_and_highlife() {
        let mut c = vec![false; 25];
        for x in 1..=3 {
            c[2 * 5 + x] = true;
        }
        let n = step(&c, 5, 5, false);
        assert!(n[5 + 2] && n[2 * 5 + 2] && n[3 * 5 + 2]);
        let mut s = vec![false; 25];
        for (x, y) in [(1, 1), (2, 1), (3, 1), (1, 2), (3, 2), (2, 3)] {
            s[y * 5 + x] = true;
        }
        assert!(!step(&s, 5, 5, false)[2 * 5 + 2]);
        assert!(step(&s, 5, 5, true)[2 * 5 + 2]);
    }
    #[test]
    fn deterministic_and_changes() {
        let a = LifeTimeline::new(32, 32, 27, true);
        let b = LifeTimeline::new(32, 32, 27, true);
        assert_eq!(a.gen_cells(10), b.gen_cells(10));
        assert_ne!(a.gen_cells(0), a.gen_cells(5));
    }
}
