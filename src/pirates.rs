//! Modo pirata: exploración a pie, tesoros, patrullas, disparos y borde del sistema.
use crate::geometry::{Mesh, cube_mesh, icosphere, load_obj, ring_mesh};
use crate::hud::{Rect, draw_text_shadow, fill_rect, stroke_rect, text_width};
use crate::math::{DVec3, Mat4, Vec3};
use crate::raster::{DrawCall, FragIn, FragmentShader, Framebuffer, RenderOptions, Stats, Uniforms, render_scene};
use crate::ship::hazard::segment_sphere_entry;
use crate::ship::{CameraRig, DeathCause, Input, NavMode, Sim};
use crate::world::{Body, cross, nearest_body, surface_height_at};

const WALK_SPEED: f64 = 13.0;
const SPRINT_SPEED: f64 = 26.0;
const BOARD_DISTANCE: f64 = 45.0;
const TREASURE_DISTANCE: f64 = 8.0;
const MAX_POLICE: usize = 5;
const BOUNDARY_WARN: f64 = 125_000.0;
const BOUNDARY_END: f64 = 165_000.0;

#[derive(Clone, Copy)]
struct Foot { body: usize, local: DVec3 }
#[derive(Clone, Copy)]
struct Treasure { local: DVec3, collected: bool }
#[derive(Clone, Copy)]
struct Police { pos: DVec3, vel: DVec3, hp: u8, cooldown: f64, kind: PoliceKind }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PoliceKind { Scout, Interceptor, Heavy }
impl PoliceKind {
    fn index(self) -> usize { match self { Self::Scout => 0, Self::Interceptor => 1, Self::Heavy => 2 } }
    fn hp(self) -> u8 { match self { Self::Scout => 1, Self::Interceptor => 2, Self::Heavy => 3 } }
    fn speed(self) -> f64 { match self { Self::Scout => 330.0, Self::Interceptor => 280.0, Self::Heavy => 220.0 } }
    fn cooldown(self) -> f64 { match self { Self::Scout => 2.1, Self::Interceptor => 1.55, Self::Heavy => 1.25 } }
    fn damage(self) -> f64 { match self { Self::Scout => 0.14, Self::Interceptor => 0.20, Self::Heavy => 0.26 } }
    fn scale(self) -> f32 { match self { Self::Scout => 1.15, Self::Interceptor => 1.45, Self::Heavy => 1.9 } }
}
fn police_limit(loot: u32) -> usize { (2 + loot as usize).min(MAX_POLICE) }
#[derive(Clone, Copy)]
struct Shot { pos: DVec3, vel: DVec3, ttl: f64, player: bool, damage: f64 }

struct ColorShader;
impl FragmentShader for ColorShader {
    fn shade(&self, f: &FragIn, _: &Uniforms) -> [f32; 3] { f.color }
}

fn push_call<'a>(calls: &mut Vec<DrawCall<'a>>, mesh: &'a Mesh, shader: &'a ColorShader, eye: DVec3, pos: DVec3, scale: f32, rotation: Mat4, time: f32) {
    let model = Mat4::translation((pos - eye).to_f32()) * rotation * Mat4::uniform_scale(scale);
    calls.push(DrawCall { mesh, model, shader, uniforms: Uniforms { light_dir: Vec3::Y, time, detail: 1.0, model } });
}

pub struct PirateGame {
    foot: Option<Foot>,
    pending_disembark: bool,
    ready_to_takeoff: bool,
    takeoff_assist: f64,
    treasures: Vec<Option<Treasure>>,
    police: Vec<Police>,
    shots: Vec<Shot>,
    police_spawn_wait: f64,
    fire_wait: f64,
    pub loot: u32,
    pub police_defeated: u32,
    pub shots_fired: u64,
    pub black_hole: Option<DVec3>,
    notice: (&'static str, f64),
    police_meshes: [Mesh; 3],
    cube: Mesh,
    sphere: Mesh,
    ring: Mesh,
    player_bullet: Mesh,
    police_bullet: Mesh,
    shader: ColorShader,
}

impl PirateGame {
    pub fn new(body_count: usize) -> Self {
        let police_meshes = ["scout", "interceptor", "heavy"].map(|name| {
            load_obj(&format!("assets/models/police_{name}.obj")).unwrap_or_else(|e| panic!("falta nave policial {name}: {e}"))
        });
        let mut cube = cube_mesh();
        for v in &mut cube.vertices { v.color = [1.0, 0.71, 0.12]; }
        let mut sphere = icosphere(2);
        for v in &mut sphere.vertices { v.color = [0.006, 0.008, 0.015]; }
        let mut ring = ring_mesh(0.75, 1.4, 48);
        for v in &mut ring.vertices { v.color = [1.0, 0.32, 0.08]; }
        let mut player_bullet = icosphere(0);
        for v in &mut player_bullet.vertices { v.color = [0.25, 1.0, 0.88]; }
        let mut police_bullet = icosphere(0);
        for v in &mut police_bullet.vertices { v.color = [1.0, 0.18, 0.18]; }
        Self { foot: None, pending_disembark: false, ready_to_takeoff: false, takeoff_assist: 0.0,
            treasures: vec![None; body_count], police: Vec::new(), shots: Vec::new(),
            police_spawn_wait: 5.0, fire_wait: 0.0, loot: 0, police_defeated: 0, shots_fired: 0, black_hole: None,
            notice: ("", 0.0), police_meshes, cube, sphere, ring, player_bullet, police_bullet, shader: ColorShader }
    }

    pub fn on_foot(&self) -> bool { self.foot.is_some() }
    pub fn goal(&self, sim: &Sim) -> u32 { sim.bodies.iter().filter(|body| body.landable).count() as u32 }
    pub fn won(&self, sim: &Sim) -> bool { self.goal(sim) > 0 && self.loot >= self.goal(sim) }
    pub fn apply_takeoff_input(&self, input: &mut Input) {
        if self.takeoff_assist > 0.0 {
            input.thrust.y = 1.0;
            input.boost = true;
        }
    }

    pub fn handle_board(&mut self, sim: &mut Sim, cam: &mut CameraRig, pressed: bool) {
        if !pressed || !sim.is_alive() { return; }
        if self.pending_disembark {
            self.pending_disembark = false;
            sim.autopilot.stop();
            self.notice = ("DESCENSO CANCELADO", 2.0);
            return;
        }
        if let Some(foot) = self.foot {
            let pos = fixed_pos(&sim.bodies[foot.body], sim.t, foot.local);
            if (pos - sim.ship.position).length() <= BOARD_DISTANCE {
                self.foot = None;
                self.ready_to_takeoff = true;
                cam.eye_override = None;
                cam.snap(&sim.ship);
                self.notice = ("A BORDO: B PARA DESPEGAR", 3.0);
            } else {
                self.notice = ("ACERCATE A TU NAVE", 2.0);
            }
            return;
        }
        match sim.ship.mode {
            NavMode::Landed if self.ready_to_takeoff => {
                self.ready_to_takeoff = false;
                self.takeoff_assist = 2.5;
                self.notice = ("DESPEGANDO: W PARA ALEJARTE", 3.0);
            }
            NavMode::Landed => self.disembark(sim, cam),
            NavMode::Orbit | NavMode::Descent => {
                if sim.land_here() {
                    self.pending_disembark = true;
                    self.notice = ("ATERRIZAJE PARA BAJAR", 3.0);
                } else { self.notice = ("NO SE PUEDE ATERRIZAR AQUI", 2.0); }
            }
            NavMode::Cruise => self.notice = ("ACERCATE A UN PLANETA", 2.0),
        }
    }

    fn disembark(&mut self, sim: &Sim, cam: &mut CameraRig) {
        let Some(i) = sim.ship.frame else { return; };
        if !sim.bodies[i].landable { return; }
        let b = &sim.bodies[i];
        let anchor = b.world_to_fixed(sim.t, sim.ship.position - b.position(sim.t));
        let up = anchor.normalized();
        let right_w = sim.ship.orientation.rotate(Vec3::X).to_f64();
        let right = b.world_to_fixed(sim.t, right_w);
        let right = tangent(right, up);
        let forward_w = sim.ship.orientation.rotate(Vec3::new(0.0, 0.0, -1.0)).to_f64();
        let forward = tangent(b.world_to_fixed(sim.t, forward_w), up);
        let local = on_surface(b, anchor + right * 35.0);
        self.foot = Some(Foot { body: i, local });
        self.ready_to_takeoff = false;
        if self.treasures[i].is_none() {
            let offset = if i % 2 == 0 { 1.0 } else { -1.0 };
            let treasure = on_surface(b, anchor + forward * 105.0 + right * (45.0 * offset));
            self.treasures[i] = Some(Treasure { local: treasure, collected: false });
        }
        self.pending_disembark = false;
        cam.eye_override = Some(fixed_pos(b, sim.t, local) + b.fixed_to_world(sim.t, local.normalized()) * 2.2);
        self.notice = if self.treasures[i].is_some_and(|t| t.collected) {
            ("TROFEO YA RECOGIDO", 3.0)
        } else {
            ("BUSCA EL TROFEO", 3.0)
        };
    }

    pub fn update(&mut self, sim: &mut Sim, cam: &mut CameraRig, input: &Input, collect: bool, fire: bool, dt: f64, old_ship_pos: DVec3) {
        let dt = dt.clamp(0.0, 0.1);
        self.notice.1 = (self.notice.1 - dt).max(0.0);
        self.fire_wait = (self.fire_wait - dt).max(0.0);
        self.police_spawn_wait = (self.police_spawn_wait - dt).max(0.0);
        self.takeoff_assist = (self.takeoff_assist - dt).max(0.0);
        if !sim.is_alive() {
            self.foot = None;
            self.pending_disembark = false;
            self.ready_to_takeoff = false;
            self.takeoff_assist = 0.0;
            cam.eye_override = None;
            self.shots.clear();
            return;
        }
        if self.pending_disembark && sim.ship.mode == NavMode::Landed { self.disembark(sim, cam); }
        if let Some(mut foot) = self.foot {
            let b = &sim.bodies[foot.body];
            let up = foot.local.normalized();
            let forward = tangent(b.world_to_fixed(sim.t, cam.orientation().rotate(Vec3::new(0.0, 0.0, -1.0)).to_f64()), up);
            let right = cross(forward, up).normalized();
            let speed = if input.boost { SPRINT_SPEED } else { WALK_SPEED };
            let movement = forward * input.thrust.z as f64 + right * input.thrust.x as f64;
            if movement.length() > 0.01 { foot.local = on_surface(b, foot.local + movement.normalized() * speed * dt); }
            let world = fixed_pos(b, sim.t, foot.local);
            let world_up = b.fixed_to_world(sim.t, foot.local.normalized());
            cam.eye_override = Some(world + world_up * 2.2);
            if collect {
                if let Some(treasure) = &mut self.treasures[foot.body] {
                    if !treasure.collected && (world - fixed_pos(b, sim.t, treasure.local)).length() <= TREASURE_DISTANCE {
                        treasure.collected = true;
                        self.loot += 1;
                        self.notice = if self.won(sim) { ("TODOS LOS TROFEOS", 4.0) } else { ("TROFEO ENCONTRADO", 4.0) };
                        self.police_spawn_wait = 0.0;
                    }
                }
            }
            self.foot = Some(foot);
            if self.won(sim) { return; }
        }

        self.update_black_hole(sim, dt, old_ship_pos);
        if !sim.is_alive() { return; }
        if fire && self.fire_wait <= 0.0 && !sim.autopilot.active {
            let dir = cam.orientation().rotate(Vec3::new(0.0, 0.0, -1.0)).to_f64().normalized();
            let origin = if self.on_foot() { cam.eye(&sim.ship) + dir * 3.0 } else { sim.ship.position + dir * 30.0 };
            self.shots.push(Shot { pos: origin, vel: dir * 2_000.0 + sim.ship.velocity * if self.on_foot() { 0.0 } else { 1.0 }, ttl: 2.0, player: true, damage: 0.0 });
            self.shots_fired += 1;
            self.fire_wait = 0.18;
        }
        self.spawn_police(sim);
        self.update_police(sim, dt);
        self.update_shots(sim, dt);
    }

    fn update_black_hole(&mut self, sim: &mut Sim, dt: f64, old_pos: DVec3) {
        let radius = sim.ship.position.length();
        if radius > BOUNDARY_WARN && self.black_hole.is_none() {
            self.black_hole = Some(sim.ship.position.normalized() * 154_000.0);
            self.notice = ("AGUJERO NEGRO: REGRESA", 4.0);
        }
        if radius < BOUNDARY_WARN - 12_000.0 { self.black_hole = None; }
        let Some(hole) = self.black_hole else { return; };
        let diff = hole - sim.ship.position;
        let dist = diff.length();
        if radius >= BOUNDARY_END || segment_sphere_entry(old_pos, sim.ship.position, hole, 2_500.0).is_some() {
            sim.kill(DeathCause::BlackHole);
            return;
        }
        let pull = (250.0 + 2_000.0 * (1.0 - dist / 35_000.0).clamp(0.0, 1.0).powi(2)).min(2_250.0);
        sim.ship.velocity += diff.normalized() * pull * dt;
    }

    fn spawn_police(&mut self, sim: &Sim) {
        let limit = police_limit(self.loot);
        if sim.autopilot.active || self.police_spawn_wait > 0.0 || self.police.len() >= limit { return; }
        let i = nearest_body(&sim.bodies, sim.ship.position, sim.t);
        if i == 0 || (sim.ship.position - sim.bodies[i].position(sim.t)).length() > sim.bodies[i].radius * 4.0 { return; }
        let b = &sim.bodies[i];
        let up = (sim.ship.position - b.position(sim.t)).normalized();
        let side = tangent(cross(up, DVec3::new(0.0, 1.0, 0.0)), up);
        for slot in self.police.len()..limit {
            let kind = match (slot + self.loot as usize) % 3 { 0 => PoliceKind::Scout, 1 => PoliceKind::Interceptor, _ => PoliceKind::Heavy };
            let angle = slot as f64 * std::f64::consts::TAU / limit as f64;
            let other = cross(up, side).normalized();
            let lateral = side * angle.cos() + other * angle.sin();
            self.police.push(Police { pos: sim.ship.position + up * (170.0 + slot as f64 * 24.0) + lateral * 290.0,
                vel: b.velocity(sim.t), hp: kind.hp(), cooldown: 1.2 + slot as f64 * 0.25, kind });
        }
        self.police_spawn_wait = (28.0 - self.loot as f64 * 3.0).max(14.0);
        self.notice = ("POLICIA EN EL RADAR", 3.0);
    }

    fn update_police(&mut self, sim: &Sim, dt: f64) {
        if sim.autopilot.active { return; }
        let on_foot = self.on_foot();
        let target = self.foot.map(|f| fixed_pos(&sim.bodies[f.body], sim.t, f.local)).unwrap_or(sim.ship.position);
        for enemy in &mut self.police {
            let i = nearest_body(&sim.bodies, enemy.pos, sim.t);
            let b = &sim.bodies[i];
            let up = (enemy.pos - b.position(sim.t)).normalized();
            let goal = target + up * if on_foot { 14.0 } else { 30.0 };
            let desired = (goal - enemy.pos).normalized() * enemy.kind.speed() + b.velocity(sim.t);
            enemy.vel += (desired - enemy.vel) * (1.0 - (-dt * 2.0).exp());
            enemy.pos += enemy.vel * dt;
            let rel = enemy.pos - b.position(sim.t);
            if b.landable {
                let floor = surface_height_at(b, sim.t, rel.normalized()) + if on_foot { 15.0 } else { 30.0 };
                if rel.length() < floor { enemy.pos = b.position(sim.t) + rel.normalized() * floor; }
            }
            enemy.cooldown -= dt;
            let to_target = target - enemy.pos;
            if enemy.cooldown <= 0.0 && to_target.length() < 600.0 {
                self.shots.push(Shot { pos: enemy.pos, vel: to_target.normalized() * 580.0, ttl: 2.0, player: false, damage: enemy.kind.damage() });
                self.shots_fired += 1;
                enemy.cooldown = enemy.kind.cooldown();
            }
        }
    }

    fn update_shots(&mut self, sim: &mut Sim, dt: f64) {
        let on_foot = self.on_foot();
        let target = self.foot.map(|f| fixed_pos(&sim.bodies[f.body], sim.t, f.local)).unwrap_or(sim.ship.position);
        for shot in &mut self.shots {
            let old = shot.pos;
            shot.pos += shot.vel * dt;
            shot.ttl -= dt;
            if shot.player {
                for enemy in &mut self.police {
                    if enemy.hp > 0 && segment_sphere_entry(old, shot.pos, enemy.pos, 26.0).is_some() {
                        enemy.hp -= 1;
                        shot.ttl = 0.0;
                        break;
                    }
                }
            } else if segment_sphere_entry(old, shot.pos, target, if on_foot { 3.0 } else { 12.0 }).is_some() {
                sim.hull = (sim.hull - shot.damage).max(0.0);
                shot.ttl = 0.0;
                if sim.hull <= 0.0 { sim.kill(DeathCause::Police); }
            }
        }
        let before = self.police.len();
        self.police.retain(|p| p.hp > 0);
        let defeated = before - self.police.len();
        if defeated > 0 {
            self.police_defeated += defeated as u32;
            self.notice = ("PATRULLA DERRIBADA", 2.0);
        }
        self.shots.retain(|s| s.ttl > 0.0);
    }

    pub fn draw_world(&self, fb: &mut Framebuffer, sim: &Sim, cam: &CameraRig, stats: &mut Stats) {
        let eye = cam.eye(&sim.ship);
        let view = cam.view_matrix();
        let proj = Mat4::perspective(crate::app::FOV_Y, fb.width as f32 / fb.height as f32, 1.0, 600_000.0);
        let mut calls = Vec::new();
        let mut push = |mesh, pos, scale, rotation| push_call(&mut calls, mesh, &self.shader, eye, pos, scale, rotation, sim.t as f32);
        if let Some(hole) = self.black_hole {
            push(&self.sphere, hole, 2_300.0, Mat4::IDENTITY);
            push(&self.ring, hole, 3_300.0, Mat4::rotation_z(0.55));
        }
        for p in &self.police {
            let heading = p.vel.normalized();
            let yaw = (-heading.x).atan2(-heading.z) as f32;
            push(&self.police_meshes[p.kind.index()], p.pos, p.kind.scale(), Mat4::rotation_y(yaw));
        }
        for (i, treasure) in self.treasures.iter().enumerate() {
            if let Some(treasure) = treasure {
                if !treasure.collected {
                    let b = &sim.bodies[i];
                    let pos = fixed_pos(b, sim.t, treasure.local);
                    if (pos - eye).length() < 2_000.0 {
                        let up = b.fixed_to_world(sim.t, treasure.local.normalized());
                        push(&self.cube, pos + up * 3.0, 5.0, Mat4::rotation_y(sim.t as f32));
                        push(&self.cube, pos + up * 13.0, 1.7, Mat4::IDENTITY);
                    }
                }
            }
        }
        for s in &self.shots { push(if s.player { &self.player_bullet } else { &self.police_bullet }, s.pos, if s.player { 2.5 } else { 3.5 }, Mat4::IDENTITY); }
        render_scene(fb, view, proj, &calls, &RenderOptions::default(), stats);
    }

    pub fn draw_hud(&self, fb: &mut Framebuffer, sim: &Sim, cam: &CameraRig) {
        let mode = if self.on_foot() { "A PIE" } else { "PIRATA ESPACIAL" };
        let line = format!("{}  TROFEOS {}/{}  POLICIAS {}", mode, self.loot, self.goal(sim), self.police_defeated);
        let x = 258;
        fill_rect(fb, Rect { x: x - 6, y: 7, w: 405, h: if self.on_foot() { 56 } else { 38 } }, [2, 8, 20], 160);
        draw_text_shadow(fb, x, 12, &line, [255, 206, 75], 1);
        if let Some(foot) = self.foot {
            let b = &sim.bodies[foot.body];
            let player = fixed_pos(b, sim.t, foot.local);
            let ship_dist = (player - sim.ship.position).length();
            let treasure = self.treasures[foot.body].and_then(|t| (!t.collected).then_some((player - fixed_pos(b, sim.t, t.local)).length()));
            let line = treasure.map(|d| format!("TROFEO A {:.0} M [E]", d)).unwrap_or_else(|| "TROFEO RECOGIDO".to_string());
            draw_text_shadow(fb, x, 28, &line, [255, 230, 120], 1);
            draw_text_shadow(fb, x, 43, &format!("NAVE A {:.0} M [B]  SHIFT CORRER", ship_dist), [170, 214, 255], 1);
        } else {
            let hint = if self.ready_to_takeoff && sim.ship.mode == NavMode::Landed {
                "B DESPEGAR  W ALEJARTE  SHIFT TURBO"
            } else { "B BAJAR  ESPACIO DISPARAR  SHIFT TURBO" };
            draw_text_shadow(fb, x, 28, hint, [170, 214, 255], 1);
        }
        self.draw_aim_and_radar(fb, sim, cam);
        if self.notice.1 > 0.0 { draw_text_shadow(fb, 288, fb.height as i32 - 85, self.notice.0, [255, 205, 65], 2); }
        if self.black_hole.is_some() { draw_text_shadow(fb, 290, 75, "HORIZONTE DEL AGUJERO NEGRO", [255, 80, 40], 1); }
    }

    fn draw_aim_and_radar(&self, fb: &mut Framebuffer, sim: &Sim, cam: &CameraRig) {
        let cx = fb.width as i32 / 2;
        let cy = fb.height as i32 / 2;
        let aim = [120, 240, 235];
        for r in [Rect { x: cx - 13, y: cy, w: 9, h: 2 }, Rect { x: cx + 5, y: cy, w: 9, h: 2 },
                  Rect { x: cx, y: cy - 13, w: 2, h: 9 }, Rect { x: cx, y: cy + 5, w: 2, h: 9 }] {
            fill_rect(fb, r, aim, 245);
        }
        fill_rect(fb, Rect { x: cx, y: cy, w: 2, h: 2 }, [255, 255, 255], 255);

        let r = Rect { x: 12, y: fb.height as i32 - 155, w: 136, h: 136 };
        fill_rect(fb, r, [3, 15, 29], 195);
        stroke_rect(fb, r, [80, 172, 210], 220);
        draw_text_shadow(fb, r.x + 8, r.y + 6, "RADAR 1.2 KM", [145, 222, 255], 1);
        let center_x = r.x + r.w / 2;
        let center_y = r.y + 77;
        fill_rect(fb, Rect { x: center_x - 1, y: center_y - 1, w: 3, h: 3 }, [110, 255, 165], 255);
        let origin = self.foot.map(|f| fixed_pos(&sim.bodies[f.body], sim.t, f.local)).unwrap_or(sim.ship.position);
        let right = cam.orientation().rotate(Vec3::X).to_f64();
        let forward = cam.orientation().rotate(Vec3::new(0.0, 0.0, -1.0)).to_f64();
        for enemy in &self.police {
            let delta = enemy.pos - origin;
            let dx = (delta.dot(right) / 1200.0 * 56.0).clamp(-56.0, 56.0) as i32;
            let dy = (delta.dot(forward) / 1200.0 * 49.0).clamp(-49.0, 49.0) as i32;
            fill_rect(fb, Rect { x: center_x + dx - 2, y: center_y - dy - 2, w: 5, h: 5 }, [255, 78, 65], 255);
        }
        draw_text_shadow(fb, r.x + 8, r.y + 117, &format!("POLICIAS {}", self.police.len()), [255, 160, 130], 1);
    }

    pub fn draw_victory(&self, fb: &mut Framebuffer, sim: &Sim) {
        let w = fb.width as i32;
        let h = fb.height as i32;
        fill_rect(fb, Rect { x: 0, y: 0, w, h }, [1, 5, 16], 195);
        let r = Rect { x: w / 2 - 260, y: h / 2 - 105, w: 520, h: 210 };
        fill_rect(fb, r, [5, 20, 43], 240);
        stroke_rect(fb, r, [255, 207, 80], 255);
        for (text, y, scale, color) in [
            ("MISION CUMPLIDA".to_string(), r.y + 22, 3, [255, 207, 80]),
            (format!("REUNISTE LOS {} TROFEOS", self.goal(sim)), r.y + 81, 2, [240, 246, 255]),
            (format!("PATRULLAS DERRIBADAS: {}", self.police_defeated), r.y + 115, 1, [175, 216, 255]),
            ("ENTER PARA JUGAR OTRA VEZ".to_string(), r.y + 155, 1, [240, 246, 255]),
            ("ESC PARA SALIR".to_string(), r.y + 177, 1, [175, 216, 255]),
        ] {
            draw_text_shadow(fb, (w - text_width(&text, scale)) / 2, y, &text, color, scale);
        }
    }
}

fn tangent(v: DVec3, up: DVec3) -> DVec3 {
    let t = v - up * v.dot(up);
    if t.length() > 1e-6 { t.normalized() } else { cross(up, DVec3::new(0.0, 1.0, 0.0)).normalized() }
}
fn on_surface(body: &Body, local: DVec3) -> DVec3 {
    let dir = local.normalized();
    dir * (body.radius * (1.0 + crate::relief::height(body.kind, dir.to_f32()) as f64) + 1.5)
}
fn fixed_pos(body: &Body, t: f64, local: DVec3) -> DVec3 { body.position(t) + body.fixed_to_world(t, local) }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::new_sim;
    #[test]
    fn treasure_stays_on_rotating_planet() {
        let sim = new_sim();
        let b = &sim.bodies[2];
        let local = on_surface(b, DVec3::new(b.radius, 0.0, 0.0));
        let p0 = fixed_pos(b, 0.0, local);
        let p1 = fixed_pos(b, 15.0, local);
        assert!((p0 - p1).length() > 10.0);
        assert!((b.world_to_fixed(15.0, p1 - b.position(15.0)) - local).length() < 1e-5);
    }
    #[test]
    fn black_hole_kills_at_boundary() {
        let mut sim = new_sim();
        sim.ship.position = DVec3::new(170_000.0, 0.0, 0.0);
        let mut game = PirateGame::new(sim.bodies.len());
        game.update_black_hole(&mut sim, 1.0 / 60.0, DVec3::new(160_000.0, 0.0, 0.0));
        assert!(!sim.is_alive());
    }
    #[test]
    fn treasure_can_be_collected_only_once_on_foot() {
        let mut sim = new_sim();
        let i = 2;
        let b = &sim.bodies[i];
        let local = on_surface(b, DVec3::new(b.radius, 0.0, 0.0));
        sim.ship.mode = NavMode::Landed;
        sim.ship.frame = Some(i);
        sim.ship.position = fixed_pos(b, sim.t, local);
        let mut game = PirateGame::new(sim.bodies.len());
        game.foot = Some(Foot { body: i, local });
        game.treasures[i] = Some(Treasure { local, collected: false });
        let mut cam = CameraRig::new();
        let ship_pos = sim.ship.position;
        for _ in 0..2 { game.update(&mut sim, &mut cam, &Input::default(), true, false, 0.0, ship_pos); }
        assert_eq!(game.loot, 1);
        assert!(game.treasures[i].unwrap().collected);
    }
    #[test]
    fn player_shots_destroy_police() {
        let mut sim = new_sim();
        let mut game = PirateGame::new(sim.bodies.len());
        let p = sim.ship.position + DVec3::new(0.0, 0.0, -100.0);
        game.police.push(Police { pos: p, vel: DVec3::ZERO, hp: 1, cooldown: 10.0, kind: PoliceKind::Scout });
        game.shots.push(Shot { pos: p + DVec3::new(0.0, 0.0, 50.0), vel: DVec3::new(0.0, 0.0, -1000.0), ttl: 2.0, player: true, damage: 0.0 });
        game.update_shots(&mut sim, 0.1);
        assert_eq!(game.police_defeated, 1);
        assert!(game.police.is_empty());
    }
    #[test]
    fn siroco_can_land_and_disembark() {
        let mut sim = new_sim();
        let i = 4;
        let b = &sim.bodies[i];
        let radial = b.position(0.0).normalized();
        sim.ship.position = b.position(0.0) + radial * (b.radius + 2500.0);
        sim.ship.velocity = b.velocity(0.0);
        sim.ship.mode = NavMode::Orbit;
        sim.ship.frame = Some(i);
        let mut game = PirateGame::new(sim.bodies.len());
        let mut cam = CameraRig::new();
        game.handle_board(&mut sim, &mut cam, true);
        assert!(game.pending_disembark);
        for _ in 0..(120 * 120) {
            let previous = sim.ship.position;
            sim.frame(1.0 / 120.0, &Input::default());
            game.update(&mut sim, &mut cam, &Input::default(), false, false, 1.0 / 120.0, previous);
            if game.on_foot() { break; }
        }
        assert!(game.on_foot(), "debe aterrizar y bajar de la nave en Siroco");
        assert!(game.treasures[i].is_some(), "Siroco debe tener un tesoro");
    }
    #[test]
    fn police_waves_grow_and_use_distinct_ships() {
        let mut sim = new_sim();
        let mut game = PirateGame::new(sim.bodies.len());
        let b = &sim.bodies[2];
        sim.ship.position = b.position(sim.t) + DVec3::new(0.0, 0.0, b.radius * 2.0);
        game.police_spawn_wait = 0.0;
        game.spawn_police(&sim);
        assert_eq!(game.police.len(), 2);
        assert_ne!(game.police[0].kind, game.police[1].kind);
        game.loot = 3;
        game.police_spawn_wait = 0.0;
        game.spawn_police(&sim);
        assert_eq!(game.police.len(), 5);
        assert!(game.police.iter().any(|p| p.kind == PoliceKind::Heavy));
        assert!(game.police.iter().all(|p| p.hp == p.kind.hp()));
    }
    #[test]
    fn fifth_unique_trophy_ends_the_game() {
        let mut sim = new_sim();
        let mut game = PirateGame::new(sim.bodies.len());
        assert_eq!(game.goal(&sim), 5);
        let mut cam = CameraRig::new();
        for i in 1..sim.bodies.len() {
            let b = &sim.bodies[i];
            let local = on_surface(b, DVec3::new(b.radius, 0.0, 0.0));
            sim.ship.position = fixed_pos(b, sim.t, local);
            game.foot = Some(Foot { body: i, local });
            game.treasures[i] = Some(Treasure { local, collected: false });
            let old_pos = sim.ship.position;
            game.update(&mut sim, &mut cam, &Input::default(), true, false, 0.0, old_pos);
            assert_eq!(game.won(&sim), i == sim.bodies.len() - 1);
        }
        assert_eq!(game.loot, 5);
    }
    #[test]
    fn boarding_then_b_starts_assisted_takeoff() {
        let mut sim = new_sim();
        let i = 2;
        let b = &sim.bodies[i];
        let local = on_surface(b, DVec3::new(b.radius, 0.0, 0.0));
        sim.ship.mode = NavMode::Landed;
        sim.ship.frame = Some(i);
        sim.ship.landed_local = local;
        sim.ship.position = fixed_pos(b, sim.t, local);
        let mut game = PirateGame::new(sim.bodies.len());
        game.foot = Some(Foot { body: i, local });
        let mut cam = CameraRig::new();
        game.handle_board(&mut sim, &mut cam, true);
        assert!(!game.on_foot() && game.ready_to_takeoff);
        game.handle_board(&mut sim, &mut cam, true);
        let mut input = Input::default();
        game.apply_takeoff_input(&mut input);
        assert!(input.thrust.y > 0.0 && input.boost);
        sim.frame(1.0 / 60.0, &input);
        assert_eq!(sim.ship.mode, NavMode::Descent);
    }
    #[test]
    fn police_can_appear_and_fire_at_landed_ship() {
        let mut sim = new_sim();
        let i = 2;
        let b = &sim.bodies[i];
        let local = on_surface(b, DVec3::new(b.radius, 0.0, 0.0));
        sim.ship.mode = NavMode::Landed;
        sim.ship.frame = Some(i);
        sim.ship.position = fixed_pos(b, sim.t, local);
        let mut game = PirateGame::new(sim.bodies.len());
        game.police_spawn_wait = 0.0;
        game.spawn_police(&sim);
        assert_eq!(game.police.len(), 2);
        for enemy in &mut game.police { enemy.cooldown = 0.0; }
        game.update_police(&sim, 0.0);
        assert!(game.shots.iter().any(|shot| !shot.player));
    }
}
