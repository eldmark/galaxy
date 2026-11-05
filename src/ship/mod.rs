//! Nave: física a paso fijo, gravedad, colisión y modos de navegación. Dueño: agente `nav`.
use crate::math::{DVec3, Quat, Vec3};
use crate::world::Body;

/// Entrada normalizada del jugador (la rellena `platform`, la consume la física).
/// Ejes de la nave: +X derecha, +Y arriba, -Z adelante.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Empuje deseado por eje en [-1, 1] (x lateral, y vertical, z: +1 = adelante).
    pub thrust: Vec3,
    /// Giro deseado en [-1, 1]: x = cabeceo, y = guiñada, z = alabeo.
    pub rotate: Vec3,
    pub boost: bool,
    pub brake: bool,
}

#[derive(Clone, Debug)]
pub struct Ship {
    pub position: DVec3,
    pub velocity: DVec3,
    pub orientation: Quat,
    pub angular_velocity: Vec3,
}

impl Ship {
    pub fn new(position: DVec3) -> Self {
        Self {
            position,
            velocity: DVec3::ZERO,
            orientation: Quat::IDENTITY,
            angular_velocity: Vec3::ZERO,
        }
    }
    /// Un paso fijo de simulación (`dt` constante, p. ej. 1/120 s). `t` = tiempo de simulación actual.
    pub fn step(&mut self, _dt: f64, _t: f64, _input: &Input, _bodies: &[Body]) {
        todo!("nav")
    }
}
