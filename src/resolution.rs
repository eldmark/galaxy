//! Escala interna del mundo; la interfaz se compone siempre a resolución nativa.
use crate::raster::Framebuffer;
use rayon::prelude::*;

pub const SCALES: [f32; 4] = [1.0, 0.85, 0.7, 0.55];

pub struct Resolution {
    pub index: usize,
    pub automatic: bool,
    slow_s: f32,
    fast_s: f32,
    avg_ms: f32,
    blocked_up_index: Option<usize>,
}

impl Resolution {
    pub fn new(fixed: Option<f32>) -> Self {
        let index = fixed.map(|s| SCALES.iter().enumerate().min_by(|a, b| (a.1 - s).abs().total_cmp(&(b.1 - s).abs())).unwrap().0).unwrap_or(0);
        Self { index, automatic: fixed.is_none(), slow_s: 0.0, fast_s: 0.0, avg_ms: 16.0, blocked_up_index: None }
    }
    pub fn scale(&self) -> f32 { SCALES[self.index] }
    pub fn dimensions(&self, w: usize, h: usize) -> (usize, usize) {
        ((w as f32 * self.scale()).round().max(1.0) as usize, (h as f32 * self.scale()).round().max(1.0) as usize)
    }
    pub fn toggle(&mut self) {
        self.automatic = !self.automatic;
        if !self.automatic { self.index = 0; }
        self.slow_s = 0.0;
        self.fast_s = 0.0;
        self.blocked_up_index = None;
    }
    pub fn update(&mut self, ms: f32, dt: f32) {
        if !self.automatic || !ms.is_finite() { return; }
        let dt = dt.clamp(0.0, 0.1);
        self.avg_ms += (ms - self.avg_ms) * (dt / 0.5).clamp(0.0, 1.0);
        self.slow_s = if self.avg_ms > 22.0 { self.slow_s + dt } else { 0.0 };
        self.fast_s = if self.avg_ms < 13.0 { self.fast_s + dt } else { 0.0 };
        if self.slow_s >= 0.5 && self.index + 1 < SCALES.len() {
            self.index += 1;
            self.blocked_up_index = Some(self.index);
            self.slow_s = 0.0;
            self.fast_s = 0.0;
        } else if self.fast_s >= 2.0 && self.index > 0 && (self.blocked_up_index != Some(self.index) || self.avg_ms < 10.5) {
            self.index -= 1;
            self.blocked_up_index = None;
            self.fast_s = 0.0;
            self.slow_s = 0.0;
        }
    }
}

/// Reescala RGBA por interpolación bilineal en CPU.
pub fn upscale(src: &Framebuffer, dst: &mut Framebuffer) {
    if src.width == 0 || src.height == 0 || dst.width == 0 || dst.height == 0 { return; }
    if src.width == dst.width && src.height == dst.height {
        dst.color.copy_from_slice(&src.color);
        return;
    }
    let sx = src.width as f32 / dst.width as f32;
    let sy = src.height as f32 / dst.height as f32;
    dst.color.par_chunks_exact_mut(dst.width * 4).enumerate().for_each(|(y, row)| {
        let fy = ((y as f32 + 0.5) * sy - 0.5).clamp(0.0, (src.height - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(src.height - 1);
        let ty = fy - y0 as f32;
        for x in 0..row.len() / 4 {
            let fx = ((x as f32 + 0.5) * sx - 0.5).clamp(0.0, (src.width - 1) as f32);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(src.width - 1);
            let tx = fx - x0 as f32;
            for c in 0..4 {
                let a = src.color[(y0 * src.width + x0) * 4 + c] as f32;
                let b = src.color[(y0 * src.width + x1) * 4 + c] as f32;
                let d = src.color[(y1 * src.width + x0) * 4 + c] as f32;
                let e = src.color[(y1 * src.width + x1) * 4 + c] as f32;
                row[x * 4 + c] = ((a + (b - a) * tx) * (1.0 - ty) + (d + (e - d) * tx) * ty).round() as u8;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upscale_edges_and_single_pixel() {
        let mut src = Framebuffer::new(1, 1);
        src.clear([17, 29, 43]);
        let mut dst = Framebuffer::new(3, 2);
        upscale(&src, &mut dst);
        assert!(dst.color.chunks_exact(4).all(|p| p == [17, 29, 43, 255]));
        let mut src = Framebuffer::new(2, 1);
        src.color = vec![0, 0, 0, 255, 200, 100, 50, 255];
        let mut dst = Framebuffer::new(3, 1);
        upscale(&src, &mut dst);
        assert_eq!(&dst.color[0..4], &[0, 0, 0, 255]);
        assert_eq!(&dst.color[4..8], &[100, 50, 25, 255]);
        assert_eq!(&dst.color[8..12], &[200, 100, 50, 255]);
    }
    #[test]
    fn hysteresis() {
        let mut r = Resolution::new(None);
        for _ in 0..40 { r.update(40.0, 1.0 / 60.0); }
        assert!(r.index > 0);
        for _ in 0..120 { r.update(17.0, 1.0 / 60.0); }
        let low = r.index;
        for _ in 0..120 { r.update(17.0, 1.0 / 60.0); }
        assert_eq!(r.index, low);
        for _ in 0..200 { r.update(5.0, 1.0 / 60.0); }
        assert!(r.index < low);
    }
    #[test]
    fn adjacent_levels_do_not_oscillate() {
        let mut r = Resolution::new(None);
        r.index = 1;
        for _ in 0..180 {
            r.update(23.0, 1.0 / 60.0);
            if r.index == 2 { break; }
        }
        assert_eq!(r.index, 2);
        for _ in 0..600 { r.update(12.0, 1.0 / 60.0); }
        assert_eq!(r.index, 2);
    }
}
