//! Matemáticas propias. Convención: mano derecha, la cámara mira hacia -Z, +Y arriba.
//! `Mat4` es row-major (`m[fila][columna]`) y transforma vectores columna: `out = M * v`.
use std::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
    pub const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length_sq(self) -> f32 {
        self.dot(self)
    }
    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }
    /// Devuelve ZERO si la longitud es casi cero (nunca produce NaN).
    pub fn normalized(self) -> Vec3 {
        let l = self.length();
        if l < 1e-12 { Vec3::ZERO } else { self * (1.0 / l) }
    }
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
    pub fn to_f64(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}
impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}

/// Vector `f64` para posiciones de mundo (precisión a decenas de km).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct DVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl DVec3 {
    pub const ZERO: DVec3 = DVec3::new(0.0, 0.0, 0.0);
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, o: DVec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn normalized(self) -> DVec3 {
        let l = self.length();
        if l < 1e-12 { DVec3::ZERO } else { self * (1.0 / l) }
    }
    pub fn to_f32(self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }
}
impl Add for DVec3 {
    type Output = DVec3;
    fn add(self, o: DVec3) -> DVec3 {
        DVec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for DVec3 {
    type Output = DVec3;
    fn sub(self, o: DVec3) -> DVec3 {
        DVec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f64> for DVec3 {
    type Output = DVec3;
    fn mul(self, s: f64) -> DVec3 {
        DVec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for DVec3 {
    type Output = DVec3;
    fn neg(self) -> DVec3 {
        DVec3::new(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for DVec3 {
    fn add_assign(&mut self, o: DVec3) {
        *self = *self + o;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };
    pub fn translation(t: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[0][3] = t.x;
        r.m[1][3] = t.y;
        r.m[2][3] = t.z;
        r
    }
    pub fn scale(s: Vec3) -> Mat4 {
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = s.x;
        r.m[1][1] = s.y;
        r.m[2][2] = s.z;
        r
    }
    pub fn uniform_scale(s: f32) -> Mat4 {
        Mat4::scale(Vec3::new(s, s, s))
    }
    pub fn rotation_x(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::IDENTITY;
        r.m[1][1] = c;
        r.m[1][2] = -s;
        r.m[2][1] = s;
        r.m[2][2] = c;
        r
    }
    pub fn rotation_y(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = c;
        r.m[0][2] = s;
        r.m[2][0] = -s;
        r.m[2][2] = c;
        r
    }
    pub fn rotation_z(a: f32) -> Mat4 {
        let (s, c) = a.sin_cos();
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = c;
        r.m[0][1] = -s;
        r.m[1][0] = s;
        r.m[1][1] = c;
        r
    }
    /// Proyección en perspectiva (cámara mira -Z). Tras dividir entre w, z_ndc ∈ [-1, 1].
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fov_y * 0.5).tan();
        let mut r = Mat4 { m: [[0.0; 4]; 4] };
        r.m[0][0] = f / aspect;
        r.m[1][1] = f;
        r.m[2][2] = (far + near) / (near - far);
        r.m[2][3] = 2.0 * far * near / (near - far);
        r.m[3][2] = -1.0;
        r
    }
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = (target - eye).normalized();
        let s = f.cross(up).normalized();
        let u = s.cross(f);
        Mat4 {
            m: [
                [s.x, s.y, s.z, -s.dot(eye)],
                [u.x, u.y, u.z, -u.dot(eye)],
                [-f.x, -f.y, -f.z, f.dot(eye)],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }
    pub fn transpose(&self) -> Mat4 {
        let mut r = Mat4 { m: [[0.0; 4]; 4] };
        for i in 0..4 {
            for j in 0..4 {
                r.m[i][j] = self.m[j][i];
            }
        }
        r
    }
    /// Devuelve (x, y, z, w) sin dividir entre w.
    pub fn transform_point4(&self, p: Vec3) -> [f32; 4] {
        let m = &self.m;
        [
            m[0][0] * p.x + m[0][1] * p.y + m[0][2] * p.z + m[0][3],
            m[1][0] * p.x + m[1][1] * p.y + m[1][2] * p.z + m[1][3],
            m[2][0] * p.x + m[2][1] * p.y + m[2][2] * p.z + m[2][3],
            m[3][0] * p.x + m[3][1] * p.y + m[3][2] * p.z + m[3][3],
        ]
    }
    /// Transforma un punto afín (asume w = 1 y no divide).
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let r = self.transform_point4(p);
        Vec3::new(r[0], r[1], r[2])
    }
    /// Transforma una dirección (ignora la traslación).
    pub fn transform_dir(&self, d: Vec3) -> Vec3 {
        let m = &self.m;
        Vec3::new(
            m[0][0] * d.x + m[0][1] * d.y + m[0][2] * d.z,
            m[1][0] * d.x + m[1][1] * d.y + m[1][2] * d.z,
            m[2][0] * d.x + m[2][1] * d.y + m[2][2] * d.z,
        )
    }
}
impl Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, o: Mat4) -> Mat4 {
        let mut r = Mat4 { m: [[0.0; 4]; 4] };
        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    r.m[i][j] += self.m[i][k] * o.m[k][j];
                }
            }
        }
        r
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quat {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Quat {
    pub const IDENTITY: Quat = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };
    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Quat {
        let a = axis.normalized();
        let (s, c) = (angle * 0.5).sin_cos();
        Quat { x: a.x * s, y: a.y * s, z: a.z * s, w: c }
    }
    pub fn normalized(self) -> Quat {
        let l = (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt();
        if l < 1e-12 {
            Quat::IDENTITY
        } else {
            Quat { x: self.x / l, y: self.y / l, z: self.z / l, w: self.w / l }
        }
    }
    pub fn conjugate(self) -> Quat {
        Quat { x: -self.x, y: -self.y, z: -self.z, w: self.w }
    }
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }
    pub fn to_mat4(self) -> Mat4 {
        let Quat { x, y, z, w } = self.normalized();
        let mut r = Mat4::IDENTITY;
        r.m[0][0] = 1.0 - 2.0 * (y * y + z * z);
        r.m[0][1] = 2.0 * (x * y - z * w);
        r.m[0][2] = 2.0 * (x * z + y * w);
        r.m[1][0] = 2.0 * (x * y + z * w);
        r.m[1][1] = 1.0 - 2.0 * (x * x + z * z);
        r.m[1][2] = 2.0 * (y * z - x * w);
        r.m[2][0] = 2.0 * (x * z - y * w);
        r.m[2][1] = 2.0 * (y * z + x * w);
        r.m[2][2] = 1.0 - 2.0 * (x * x + y * y);
        r
    }
}
impl Mul for Quat {
    type Output = Quat;
    /// `a * b` aplica primero `b` y luego `a`.
    fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn perspective_maps_near_and_far_to_ndc_range() {
        let p = Mat4::perspective(1.0, 1.5, 0.5, 100.0);
        let n = p.transform_point4(Vec3::new(0.0, 0.0, -0.5));
        let f = p.transform_point4(Vec3::new(0.0, 0.0, -100.0));
        assert!((n[2] / n[3] + 1.0).abs() < 1e-4);
        assert!((f[2] / f[3] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn look_at_puts_target_on_negative_z() {
        let v = Mat4::look_at(Vec3::new(3.0, 2.0, 5.0), Vec3::ZERO, Vec3::Y);
        let t = v.transform_point(Vec3::ZERO);
        assert!(t.x.abs() < 1e-4 && t.y.abs() < 1e-4 && t.z < 0.0);
        assert!((t.z + Vec3::new(3.0, 2.0, 5.0).length()).abs() < 1e-4);
    }

    #[test]
    fn quat_matches_matrix_rotation() {
        let q = Quat::from_axis_angle(Vec3::Y, 0.7);
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!(close(q.rotate(v), Mat4::rotation_y(0.7).transform_dir(v)));
        assert!(close(q.to_mat4().transform_dir(v), q.rotate(v)));
    }

    #[test]
    fn matrix_product_composes_right_to_left() {
        let m = Mat4::translation(Vec3::new(1.0, 0.0, 0.0)) * Mat4::uniform_scale(2.0);
        assert!(close(m.transform_point(Vec3::new(1.0, 1.0, 1.0)), Vec3::new(3.0, 2.0, 2.0)));
    }

    #[test]
    fn normalized_zero_is_zero_not_nan() {
        assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
    }
}
