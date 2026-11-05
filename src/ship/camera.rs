//! Cámara de nave con origen flotante: la matriz de vista NO lleva traslación; cada objeto se dibuja con
//! `(posición_mundo - ojo)` calculado en f64 y luego pasado a f32 (`CameraRig::relative`). Así no hay
//! vibración por precisión a decenas de km.
use super::Ship;
use crate::math::{DVec3, Mat4, Quat, Vec3};

/// Matriz de vista (sin traslación) a partir de la orientación de la cámara: inversa de la rotación.
pub fn view_from_orientation(q: Quat) -> Mat4 {
    q.normalized().to_mat4().transpose()
}

/// Posición del ojo en el mundo: nave + desplazamiento en espacio nave rotado por la orientación de la cámara.
pub fn eye_position(ship_pos: DVec3, cam_orientation: Quat, offset: Vec3) -> DVec3 {
    ship_pos + cam_orientation.rotate(offset).to_f64()
}

#[derive(Clone, Debug)]
pub struct CameraRig {
    /// Orientación suavizada que sigue a la nave con retardo.
    pub follow: Quat,
    /// Desplazamiento del ojo en espacio nave (x derecha, y arriba, z atrás). (0,0,0) = cabina.
    pub offset: Vec3,
    /// Mirada libre (no cambia el rumbo de la nave).
    pub look_yaw: f32,
    pub look_pitch: f32,
    /// Rapidez de seguimiento (1/s) y de retorno de la mirada libre al soltar.
    pub follow_rate: f32,
    pub look_return_rate: f32,
    /// Radianes por píxel de ratón.
    pub sensitivity: f32,
}

impl CameraRig {
    pub fn new() -> Self {
        Self {
            follow: Quat::IDENTITY,
            offset: Vec3::new(0.0, 7.0, 26.0),
            look_yaw: 0.0,
            look_pitch: 0.0,
            follow_rate: 7.0,
            look_return_rate: 5.0,
            sensitivity: 0.004,
        }
    }
    /// Copia la orientación de la nave sin retardo (al iniciar o cambiar de cámara).
    pub fn snap(&mut self, ship: &Ship) {
        self.follow = ship.orientation;
        self.look_yaw = 0.0;
        self.look_pitch = 0.0;
    }
    /// Se llama una vez por cuadro con el tiempo real: el seguimiento usa `1 - e^(-rate·dt)` (independiente de FPS).
    /// `mouse_dx/dy` en píxeles; `free_look` = botón derecho pulsado.
    pub fn update(&mut self, frame_dt: f64, ship: &Ship, mouse_dx: f32, mouse_dy: f32, free_look: bool) {
        let k = 1.0 - (-(self.follow_rate as f64) * frame_dt).exp() as f32;
        let a = self.follow;
        let mut b = ship.orientation;
        if a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w < 0.0 {
            b = Quat { x: -b.x, y: -b.y, z: -b.z, w: -b.w };
        }
        self.follow = Quat {
            x: a.x + (b.x - a.x) * k,
            y: a.y + (b.y - a.y) * k,
            z: a.z + (b.z - a.z) * k,
            w: a.w + (b.w - a.w) * k,
        }
        .normalized();
        if free_look {
            self.look_yaw -= mouse_dx * self.sensitivity;
            self.look_pitch = (self.look_pitch - mouse_dy * self.sensitivity).clamp(-1.45, 1.45);
        } else {
            let d = (-(self.look_return_rate as f64) * frame_dt).exp() as f32;
            self.look_yaw *= d;
            self.look_pitch *= d;
        }
    }
    /// Orientación final de la cámara = seguimiento * mirada libre.
    pub fn orientation(&self) -> Quat {
        (self.follow * Quat::from_axis_angle(Vec3::Y, self.look_yaw) * Quat::from_axis_angle(Vec3::X, self.look_pitch))
            .normalized()
    }
    /// Posición del ojo en el mundo (f64).
    pub fn eye(&self, ship: &Ship) -> DVec3 {
        eye_position(ship.position, self.orientation(), self.offset)
    }
    /// Matriz de vista relativa a la cámara (el ojo está en el origen).
    pub fn view_matrix(&self) -> Mat4 {
        view_from_orientation(self.orientation())
    }
    /// Posición de un objeto relativa al ojo, restada en f64 y convertida a f32 (origen flotante).
    pub fn relative(eye: DVec3, world: DVec3) -> Vec3 {
        (world - eye).to_f32()
    }
    /// Dirección del suelo de la nave (+Y de la cámara) por si el HUD la necesita.
    pub fn up(&self) -> Vec3 {
        self.orientation().rotate(Vec3::Y)
    }
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_is_inverse_rotation_and_relative() {
        let mut rig = CameraRig::new();
        let mut ship = Ship::new(DVec3::new(100_000.0, 5.0, -3.0));
        ship.orientation = Quat::from_axis_angle(Vec3::Y, 0.8);
        rig.snap(&ship);
        let eye = rig.eye(&ship);
        // un objeto justo delante de la nave se ve en -Z de la vista
        let ahead = ship.position + ship.orientation.rotate(Vec3::new(0.0, 0.0, -100.0)).to_f64();
        let v = rig.view_matrix().transform_point(CameraRig::relative(eye, ahead));
        assert!(v.z < 0.0 && v.x.abs() < 1.0);
    }

    #[test]
    fn free_look_returns_and_does_not_touch_ship() {
        let mut rig = CameraRig::new();
        let ship = Ship::new(DVec3::ZERO);
        rig.update(0.016, &ship, 100.0, 0.0, true);
        assert!(rig.look_yaw.abs() > 0.1);
        for _ in 0..200 {
            rig.update(0.016, &ship, 0.0, 0.0, false);
        }
        assert!(rig.look_yaw.abs() < 1e-3);
        assert_eq!(ship.orientation, Quat::IDENTITY);
    }
}
