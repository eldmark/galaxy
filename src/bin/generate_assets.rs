//! Modelado paramétrico de todos los OBJ del juego. Ejecutar con `cargo run --bin generate_assets`.
//! La geometría nace de vectores y triángulos aquí; el juego solo carga los archivos resultantes.
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

type P = [f64; 3];
type C = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn normal(a: P) -> P {
    let n = dot(a, a).sqrt().max(1e-12);
    [a[0] / n, a[1] / n, a[2] / n]
}
fn center(v: &[P]) -> P {
    let mut c = [0.0; 3];
    for p in v {
        for k in 0..3 {
            c[k] += p[k];
        }
    }
    for x in &mut c {
        *x /= v.len() as f64;
    }
    c
}

struct Part {
    vertices: Vec<P>,
    faces: Vec<[usize; 3]>,
}
fn loft(rings: Vec<Vec<P>>, caps: bool) -> Part {
    assert!(rings.len() >= 2 && rings.iter().all(|r| r.len() == rings[0].len()));
    let n = rings[0].len();
    let mut vertices: Vec<P> = rings.iter().flatten().copied().collect();
    let mut faces = Vec::new();
    for j in 0..rings.len() - 1 {
        for i in 0..n {
            let a = j * n + i;
            let b = j * n + (i + 1) % n;
            faces.push([a, b, b + n]);
            faces.push([a, b + n, a + n]);
        }
    }
    if caps {
        for j in [0, rings.len() - 1] {
            let c = center(&rings[j]);
            if rings[j].iter().all(|p| dot(sub(*p, c), sub(*p, c)) < 1e-12) {
                continue;
            }
            let ci = vertices.len();
            vertices.push(c);
            for i in 0..n {
                faces.push([ci, j * n + i, j * n + (i + 1) % n]);
            }
        }
    }
    Part { vertices, faces }
}
fn ring(z: f64, hw: f64, hh: f64, yc: f64, chamfer: f64, xc: f64, n: usize) -> Vec<P> {
    if n == 4 {
        vec![
            [xc + hw, yc + hh, z],
            [xc - hw, yc + hh, z],
            [xc - hw, yc - hh, z],
            [xc + hw, yc - hh, z],
        ]
    } else {
        vec![
            [xc + hw - chamfer, yc + hh, z],
            [xc - hw + chamfer, yc + hh, z],
            [xc - hw, yc + hh - chamfer, z],
            [xc - hw, yc - hh + chamfer, z],
            [xc - hw + chamfer, yc - hh, z],
            [xc + hw - chamfer, yc - hh, z],
            [xc + hw, yc - hh + chamfer, z],
            [xc + hw, yc + hh - chamfer, z],
        ]
    }
}
fn circ(z: f64, radius: f64, n: usize, x: f64, y: f64) -> Vec<P> {
    (0..n)
        .map(|i| {
            let a = TAU * i as f64 / n as f64;
            [x + radius * a.cos(), y + radius * a.sin(), z]
        })
        .collect()
}
fn circ_y(y: f64, radius: f64, n: usize, x: f64, z: f64, rotation: f64) -> Vec<P> {
    (0..n)
        .map(|i| {
            let a = rotation + TAU * i as f64 / n as f64;
            [x + radius * a.cos(), y, z + radius * a.sin()]
        })
        .collect()
}
fn apex(x: f64, y: f64, z: f64, n: usize) -> Vec<P> {
    vec![[x, y, z]; n]
}
fn cuboid(x0: f64, x1: f64, y0: f64, y1: f64, z0: f64, z1: f64) -> Part {
    loft(
        vec![
            ring(
                z0,
                (x1 - x0) / 2.0,
                (y1 - y0) / 2.0,
                (y0 + y1) / 2.0,
                0.0,
                (x0 + x1) / 2.0,
                4,
            ),
            ring(
                z1,
                (x1 - x0) / 2.0,
                (y1 - y0) / 2.0,
                (y0 + y1) / 2.0,
                0.0,
                (x0 + x1) / 2.0,
                4,
            ),
        ],
        true,
    )
}

struct Face {
    v: [usize; 3],
    group: usize,
}
#[derive(Default)]
struct Model {
    vertices: Vec<P>,
    colors: Vec<C>,
    faces: Vec<Face>,
    groups: Vec<&'static str>,
}
impl Model {
    fn add(&mut self, group: &'static str, part: Part, color: C, jitter: f64) {
        let gi = if let Some(i) = self.groups.iter().position(|g| *g == group) {
            i
        } else {
            self.groups.push(group);
            self.groups.len() - 1
        };
        let mut unique = Vec::new();
        let mut old_to_unique = Vec::with_capacity(part.vertices.len());
        let mut keys = HashMap::new();
        for p in &part.vertices {
            let key = (
                (p[0] * 1e5).round() as i64,
                (p[1] * 1e5).round() as i64,
                (p[2] * 1e5).round() as i64,
            );
            let id = *keys.entry(key).or_insert_with(|| {
                unique.push([key.0 as f64 / 1e5, key.1 as f64 / 1e5, key.2 as f64 / 1e5]);
                unique.len() - 1
            });
            old_to_unique.push(id);
        }
        let c = center(&unique);
        let mut local = HashMap::new();
        for f in &part.faces {
            let a = part.vertices[f[0]];
            let b = part.vertices[f[1]];
            let d = part.vertices[f[2]];
            let n = cross(sub(b, a), sub(d, a));
            if dot(n, n) < 1e-12 {
                continue;
            }
            let fc = [
                (a[0] + b[0] + d[0]) / 3.0,
                (a[1] + b[1] + d[1]) / 3.0,
                (a[2] + b[2] + d[2]) / 3.0,
            ];
            let mut ids = [f[0], f[1], f[2]];
            if dot(n, sub(fc, c)) < 0.0 {
                ids.swap(1, 2);
            }
            let mut out = [0; 3];
            for (k, old) in ids.iter().enumerate() {
                let uid = old_to_unique[*old];
                out[k] = *local.entry(uid).or_insert_with(|| {
                    let p = unique[uid];
                    let h = (p[0] * 12.9898 + p[1] * 78.233 + p[2] * 37.719).sin() * 43758.5453;
                    let h = h - h.floor() - 0.5;
                    self.vertices.push(p);
                    self.colors
                        .push(color.map(|v| (v + h * jitter * 2.0).clamp(0.0, 1.0)));
                    self.vertices.len() - 1
                });
            }
            self.faces.push(Face { v: out, group: gi });
        }
    }
    fn write(&self, path: &Path, title: &str) -> io::Result<()> {
        let mut file = File::create(path)?;
        writeln!(
            file,
            "# galaxy - {title}\n# unidades: metros; avance -Z, +Y arriba, +X derecha\n# v x y z r g b ; triangulos antihorarios; vn por cara"
        )?;
        for (p, c) in self.vertices.iter().zip(&self.colors) {
            writeln!(
                file,
                "v {:0.4} {:0.4} {:0.4} {:0.3} {:0.3} {:0.3}",
                p[0], p[1], p[2], c[0], c[1], c[2]
            )?;
        }
        for f in &self.faces {
            let a = self.vertices[f.v[0]];
            let b = self.vertices[f.v[1]];
            let c = self.vertices[f.v[2]];
            let n = normal(cross(sub(b, a), sub(c, a)));
            writeln!(file, "vn {:0.4} {:0.4} {:0.4}", n[0], n[1], n[2])?;
        }
        for (gi, group) in self.groups.iter().enumerate() {
            writeln!(file, "o {group}")?;
            for (fi, face) in self.faces.iter().enumerate() {
                if face.group == gi {
                    writeln!(
                        file,
                        "f {}//{} {}//{} {}//{}",
                        face.v[0] + 1,
                        fi + 1,
                        face.v[1] + 1,
                        fi + 1,
                        face.v[2] + 1,
                        fi + 1
                    )?;
                }
            }
        }
        println!(
            "{}: {} triángulos, {} vértices",
            path.display(),
            self.faces.len(),
            self.vertices.len()
        );
        Ok(())
    }
}

const HULL: C = [0.55, 0.52, 0.47];
const HULL_D: C = [0.38, 0.36, 0.33];
const ORANGE: C = [0.86, 0.42, 0.10];
const GLASS: C = [0.10, 0.17, 0.38];
const DARK: C = [0.06, 0.06, 0.07];
const EMIT: C = [1.0, 0.92, 0.65];
const METAL: C = [0.30, 0.30, 0.32];

fn ship(high: bool) -> Model {
    let mut m = Model::default();
    let n = if high { 8 } else { 4 };
    let cr = if high { 0.45 } else { 0.0 };
    let ns = if high { 8 } else { 5 };
    let hull = vec![
        ring(
            -5.9,
            0.55,
            0.40,
            -0.25,
            if high { 0.15 } else { 0.0 },
            0.0,
            n,
        ),
        ring(-4.0, 1.8, 0.95, -0.1, cr, 0.0, n),
        ring(-1.2, 2.7, 1.30, 0.0, cr, 0.0, n),
        ring(3.0, 2.8, 1.30, 0.0, cr, 0.0, n),
        ring(4.9, 2.3, 1.10, 0.0, cr * 0.8, 0.0, n),
    ];
    m.add(
        "hull",
        loft(
            if high {
                hull
            } else {
                vec![hull[0].clone(), hull[2].clone(), hull[4].clone()]
            },
            true,
        ),
        HULL,
        0.04,
    );
    let cab = vec![
        ring(-5.1, 0.6, 0.15, 0.55, 0.0, 0.0, 4),
        ring(-4.2, 1.2, 0.5, 1.15, 0.0, 0.0, 4),
        ring(-2.6, 1.45, 0.65, 1.55, 0.0, 0.0, 4),
        ring(-1.3, 1.5, 0.4, 1.3, 0.0, 0.0, 4),
    ];
    m.add(
        "cockpit",
        loft(
            if high {
                cab
            } else {
                vec![cab[0].clone(), cab[2].clone(), cab[3].clone()]
            },
            true,
        ),
        GLASS,
        0.02,
    );
    m.add(
        "hull",
        cuboid(-1.6, 1.6, 1.2, 1.4, -1.5, -0.9),
        ORANGE,
        0.04,
    );
    for s in [-1.0, 1.0] {
        let r = vec![
            [2.4, 0.35, -1.8],
            [2.4, 0.35, 4.6],
            [2.4, -0.35, 4.6],
            [2.4, -0.35, -1.8],
        ];
        let t = vec![
            [6.3, 0.18, 2.2],
            [6.3, 0.18, 4.6],
            [6.3, -0.12, 4.6],
            [6.3, -0.12, 2.2],
        ];
        m.add(
            "hull",
            loft(
                vec![
                    r.iter().map(|p| [s * p[0], p[1], p[2]]).collect(),
                    t.iter().map(|p| [s * p[0], p[1], p[2]]).collect(),
                ],
                true,
            ),
            HULL_D,
            0.04,
        );
        let (a, b) = ordered(s * 6.3, s * 6.9);
        m.add("hull", cuboid(a, b, -0.14, 0.2, 2.2, 4.6), ORANGE, 0.04);
        if high {
            let (a, b) = ordered(s * 6.4, s * 6.6);
            m.add("hull", cuboid(a, b, 0.1, 0.9, 3.4, 4.7), ORANGE, 0.04);
        }
    }
    for s in [-1.0, 1.0] {
        let x = s * 1.65;
        let nac = vec![
            circ(1.0, 0.85, ns, x, 0.1),
            circ(3.0, 1.0, ns, x, 0.1),
            circ(5.4, 0.95, ns, x, 0.1),
        ];
        m.add(
            "hull",
            loft(
                if high {
                    nac
                } else {
                    vec![nac[0].clone(), nac[2].clone()]
                },
                true,
            ),
            HULL_D,
            0.04,
        );
        m.add(
            "hull",
            loft(
                vec![circ(5.2, 1.0, ns, x, 0.1), circ(5.5, 1.0, ns, x, 0.1)],
                true,
            ),
            ORANGE,
            0.04,
        );
        m.add(
            "nozzle",
            loft(
                vec![circ(5.5, 0.9, ns, x, 0.1), circ(6.3, 0.72, ns, x, 0.1)],
                false,
            ),
            DARK,
            0.01,
        );
        m.add(
            "nozzle",
            loft(
                vec![circ(6.15, 0.64, ns, x, 0.1), circ(6.2, 0.64, ns, x, 0.1)],
                true,
            ),
            EMIT,
            0.0,
        );
    }
    m.add("hull", cuboid(-0.2, 0.2, 1.0, 1.6, 2.6, 4.8), HULL_D, 0.04);
    if high {
        for s in [-1.0, 1.0] {
            let (a, b) = ordered(s * 2.7, s * 3.3);
            m.add("hull", cuboid(a, b, -0.6, 0.5, -1.0, 2.4), HULL_D, 0.04);
            let (a, b) = ordered(s * 3.3, s * 3.36);
            m.add("hull", cuboid(a, b, -0.3, 0.2, -0.6, 1.0), ORANGE, 0.04);
            m.add(
                "hull",
                cuboid(s * 1.0 - 0.4, s * 1.0 + 0.4, 1.28, 1.45, 0.3, 2.6),
                HULL_D,
                0.04,
            );
        }
        for s in [-1.0, 1.0] {
            for z in [1.5, 2.3, 3.1, 3.9] {
                m.add(
                    "hull",
                    loft(
                        vec![
                            circ(z, 1.06, ns, s * 1.65, 0.1),
                            circ(z + 0.18, 1.06, ns, s * 1.65, 0.1),
                        ],
                        true,
                    ),
                    METAL,
                    0.04,
                );
            }
            m.add(
                "hull",
                cuboid(s * 1.65 - 0.12, s * 1.65 + 0.12, 1.0, 1.9, 4.0, 5.3),
                ORANGE,
                0.04,
            );
            m.add(
                "hull",
                cuboid(s * 0.5 - 0.3, s * 0.5 + 0.3, -0.9, -0.3, -5.2, -4.4),
                HULL_D,
                0.04,
            );
        }
        let dome = [0.0_f64, 0.5, 1.0, 1.3]
            .map(|a| circ_y(-1.3 - 0.5 * a.sin(), 0.6 * a.cos(), 8, 0.0, -1.6, 0.0));
        m.add("hull", loft(dome.to_vec(), true), METAL, 0.04);
        for (x, z, w, d) in [
            (-1.0_f64, -3.0, 0.8, 0.7),
            (1.2, -2.5, 0.6, 0.9),
            (-0.6, 0.5, 0.9, 0.6),
            (0.9, 2.0, 0.7, 0.8),
            (-1.8, 3.0, 0.6, 0.5),
            (1.7, 4.0, 0.6, 0.6),
            (-2.3, -1.0, 0.5, 0.9),
            (2.3, 0.2, 0.5, 0.7),
        ] {
            let y = if x.abs() < 2.0 { 1.2 } else { 0.9 };
            m.add(
                "hull",
                cuboid(
                    x - w / 2.0,
                    x + w / 2.0,
                    y,
                    y + 0.07,
                    z - d / 2.0,
                    z + d / 2.0,
                ),
                HULL_D,
                0.04,
            );
        }
        m.add(
            "hull",
            cuboid(-1.2, 1.2, -1.6, -1.25, -3.0, 2.0),
            METAL,
            0.04,
        );
        m.add(
            "hull",
            cuboid(-0.05, 0.05, 1.3, 2.2, -0.8, -0.7),
            METAL,
            0.04,
        );
        m.add(
            "hull",
            cuboid(-0.9, 0.9, -0.12, 0.05, -5.0, -4.6),
            ORANGE,
            0.04,
        );
    }
    for (group, x, z, pad) in [
        ("gear_left", -2.0, 2.0, 0.7),
        ("gear_right", 2.0, 2.0, 0.7),
        ("gear_front", 0.0, -3.2, 0.55),
    ] {
        m.add(
            group,
            cuboid(x - 0.28, x + 0.28, -1.5, -0.8, z - 0.35, z + 0.35),
            HULL_D,
            0.04,
        );
        m.add(
            group,
            loft(
                vec![
                    circ_y(-1.0, 0.13, 5, x, z, 0.0),
                    circ_y(-3.0, 0.11, 5, x, z, 0.0),
                ],
                true,
            ),
            METAL,
            0.04,
        );
        m.add(
            group,
            cuboid(x - pad / 2.0, x + pad / 2.0, -3.3, -3.0, z - 0.55, z + 0.55),
            ORANGE,
            0.04,
        );
    }
    m
}
fn ordered(a: f64, b: f64) -> (f64, f64) {
    if a < b { (a, b) } else { (b, a) }
}

fn tree() -> Model {
    let mut m = Model::default();
    m.add(
        "trunk",
        loft(
            vec![
                circ_y(0.0, 0.55, 6, 0.0, 0.0, 0.0),
                circ_y(1.2, 0.38, 6, 0.05, 0.0, 0.0),
                circ_y(3.5, 0.26, 6, 0.0, 0.05, 0.0),
            ],
            true,
        ),
        [0.30, 0.20, 0.12],
        0.03,
    );
    let greens = [
        [0.12, 0.34, 0.14],
        [0.16, 0.42, 0.16],
        [0.20, 0.48, 0.18],
        [0.24, 0.54, 0.20],
        [0.30, 0.60, 0.22],
        [0.36, 0.66, 0.26],
    ];
    for (i, col) in greens.into_iter().enumerate() {
        let y = 2.0 + i as f64 * 1.15;
        let r = 2.6 - i as f64 * 0.4;
        let rot = i as f64 * 0.35;
        m.add(
            "canopy",
            loft(
                vec![
                    circ_y(y, r, 9, 0.0, 0.0, rot),
                    circ_y(y + 0.55, r * 0.55, 9, 0.0, 0.0, rot),
                    apex(0.0, y + 1.9, 0.0, 9),
                ],
                true,
            ),
            col,
            0.05,
        );
    }
    m
}
fn spire(m: &mut Model, x: f64, z: f64, h: f64, r: f64, lean: f64, rot: f64, col: C) {
    m.add(
        "spire",
        loft(
            vec![
                circ_y(0.0, r, 6, x, z, rot),
                circ_y(h * 0.3, r * 0.85, 6, x + lean * 0.3, z, rot + 0.2),
                circ_y(h * 0.75, r * 0.5, 6, x + lean * 0.8, z, rot + 0.4),
                apex(x + lean, h, z, 6),
            ],
            true,
        ),
        col,
        0.03,
    );
}
fn obsidian() -> Model {
    let mut m = Model::default();
    spire(&mut m, 0.0, 0.0, 5.0, 0.95, 0.4, 0.1, [0.07, 0.05, 0.11]);
    spire(&mut m, 1.1, 0.4, 3.2, 0.6, 0.5, 0.5, [0.09, 0.06, 0.14]);
    spire(&mut m, -0.9, 0.7, 2.6, 0.55, -0.4, 0.9, [0.06, 0.04, 0.10]);
    spire(&mut m, 0.3, -1.0, 1.8, 0.45, 0.2, 0.3, [0.12, 0.07, 0.15]);
    m.add(
        "shard",
        loft(
            vec![
                circ_y(0.0, 0.5, 6, -0.6, -0.9, 0.2),
                circ_y(0.5, 0.35, 6, -0.6, -0.9, 0.2),
                apex(-0.5, 1.2, -0.9, 6),
            ],
            true,
        ),
        [0.35, 0.10, 0.08],
        0.03,
    );
    m
}
fn crystal(m: &mut Model, x: f64, z: f64, h: f64, r: f64, lean: f64, rot: f64, col: C) {
    m.add(
        "crystal",
        loft(
            vec![
                circ_y(0.0, r * 0.7, 6, x, z, rot),
                circ_y(h * 0.15, r, 6, x + lean * 0.1, z, rot),
                circ_y(h * 0.7, r * 0.9, 6, x + lean * 0.7, z, rot),
                apex(x + lean, h, z, 6),
            ],
            true,
        ),
        col,
        0.04,
    );
}
fn ice_crystal() -> Model {
    let mut m = Model::default();
    crystal(&mut m, 0.0, 0.0, 4.0, 0.7, 0.3, 0.0, [0.62, 0.86, 0.95]);
    crystal(&mut m, 0.9, 0.3, 2.6, 0.5, 0.7, 0.4, [0.70, 0.90, 0.98]);
    crystal(&mut m, -0.8, 0.5, 2.0, 0.45, -0.6, 0.8, [0.55, 0.80, 0.92]);
    crystal(&mut m, 0.2, -0.9, 1.5, 0.35, 0.3, 0.2, [0.80, 0.94, 1.0]);
    m
}
fn building() -> Model {
    let mut m = Model::default();
    let cream = [0.92, 0.90, 0.84];
    let dark = [0.10, 0.10, 0.13];
    let pink = [0.95, 0.60, 0.68];
    let stone = [0.42, 0.44, 0.52];
    for (group, cub, color, jitter) in [
        ("base", [-3.0, 3.0, 0.0, 1.5, -3.0, 3.0], stone, 0.02),
        ("head", [-2.5, 2.5, 1.5, 6.0, -2.25, 2.25], cream, 0.02),
        ("patch", [-2.55, -0.6, 3.6, 5.6, -0.4, 2.3], dark, 0.02),
        ("ear", [-3.4, -2.5, 5.0, 6.4, -0.8, 0.8], cream, 0.02),
        ("ear", [2.5, 3.4, 5.0, 6.4, -0.8, 0.8], cream, 0.02),
        ("horn", [-2.2, -1.2, 6.0, 8.0, -0.5, 0.5], dark, 0.02),
        ("horn", [1.2, 2.2, 6.0, 8.0, -0.5, 0.5], dark, 0.02),
        ("snout", [-1.6, 1.6, 1.5, 3.3, 2.25, 3.4], pink, 0.02),
        ("nostril", [-1.0, -0.4, 2.0, 2.7, 3.4, 3.5], dark, 0.0),
        ("nostril", [0.4, 1.0, 2.0, 2.7, 3.4, 3.5], dark, 0.0),
        ("eye", [-1.9, -1.1, 4.3, 5.1, 2.25, 2.35], dark, 0.0),
        ("eye", [1.1, 1.9, 4.3, 5.1, 2.25, 2.35], dark, 0.0),
    ] {
        m.add(
            group,
            cuboid(cub[0], cub[1], cub[2], cub[3], cub[4], cub[5]),
            color,
            jitter,
        );
    }
    m
}

const BLUE: C = [0.12, 0.35, 0.78];
const POLICE_DARK: C = [0.035, 0.08, 0.20];
const LIGHT: C = [0.24, 0.68, 1.0];
const POLICE_GLASS: C = [0.05, 0.15, 0.27];
const WHITE: C = [0.82, 0.92, 1.0];

fn police_scout() -> Model {
    let mut m = Model::default();
    m.add(
        "fuselaje",
        loft(
            vec![
                ring(-5.0, 0.25, 0.25, 0.0, 0.0, 0.0, 4),
                ring(-2.0, 1.1, 0.55, 0.0, 0.0, 0.0, 4),
                ring(3.2, 0.9, 0.55, 0.0, 0.0, 0.0, 4),
                ring(4.4, 0.3, 0.3, 0.0, 0.0, 0.0, 4),
            ],
            true,
        ),
        BLUE,
        0.0,
    );
    m.add(
        "cabina",
        cuboid(-0.65, 0.65, 0.52, 0.93, -2.3, -0.2),
        POLICE_GLASS,
        0.0,
    );
    for s in [-1.0, 1.0] {
        let (a, b) = ordered(s * 0.8, s * 5.2);
        m.add(
            "ala_delta",
            loft(
                vec![
                    ring(-1.1, b - a, 0.14, 0.0, 0.0, (a + b) / 2.0, 4),
                    ring(2.5, 0.4, 0.14, 0.0, 0.0, s * 1.2, 4),
                ],
                true,
            ),
            LIGHT,
            0.0,
        );
        let (a, b) = ordered(s * 1.3, s * 2.0);
        m.add(
            "motor",
            cuboid(a, b, -0.45, 0.35, 2.6, 4.2),
            POLICE_DARK,
            0.0,
        );
        m.add(
            "motor",
            cuboid(a + 0.1, b - 0.1, -0.25, 0.2, 4.2, 4.28),
            WHITE,
            0.0,
        );
    }
    m
}
fn police_interceptor() -> Model {
    let mut m = Model::default();
    m.add(
        "fuselaje",
        loft(
            vec![
                ring(-4.5, 0.35, 0.35, 0.0, 0.0, 0.0, 4),
                ring(-1.4, 1.5, 0.7, 0.0, 0.0, 0.0, 4),
                ring(3.3, 1.45, 0.7, 0.0, 0.0, 0.0, 4),
                ring(4.5, 0.7, 0.4, 0.0, 0.0, 0.0, 4),
            ],
            true,
        ),
        POLICE_DARK,
        0.0,
    );
    m.add(
        "cabina",
        cuboid(-0.8, 0.8, 0.7, 1.12, -2.2, 0.0),
        POLICE_GLASS,
        0.0,
    );
    for s in [-1.0, 1.0] {
        let (a, b) = ordered(s * 1.3, s * 4.8);
        m.add("ala", cuboid(a, b, -0.18, 0.18, -0.5, 2.5), BLUE, 0.0);
        let (a, b) = ordered(s * 4.1, s * 4.8);
        m.add("punta", cuboid(a, b, -0.45, 0.8, -1.9, 2.8), LIGHT, 0.0);
        let (a, b) = ordered(s * 1.65, s * 2.6);
        m.add("motor", cuboid(a, b, -0.6, 0.3, 3.3, 4.8), BLUE, 0.0);
        m.add(
            "motor",
            cuboid(a + 0.1, b - 0.1, -0.42, 0.12, 4.8, 4.88),
            WHITE,
            0.0,
        );
    }
    m
}
fn police_heavy() -> Model {
    let mut m = Model::default();
    m.add(
        "blindaje",
        loft(
            vec![
                ring(-4.0, 0.9, 0.7, 0.0, 0.0, 0.0, 4),
                ring(-2.5, 2.2, 1.0, 0.0, 0.0, 0.0, 4),
                ring(3.4, 2.4, 1.0, 0.0, 0.0, 0.0, 4),
                ring(4.8, 1.8, 0.8, 0.0, 0.0, 0.0, 4),
            ],
            true,
        ),
        BLUE,
        0.0,
    );
    m.add(
        "cabina",
        cuboid(-1.0, 1.0, 1.0, 1.38, -3.2, -0.8),
        POLICE_GLASS,
        0.0,
    );
    for s in [-1.0, 1.0] {
        let (a, b) = ordered(s * 2.2, s * 5.5);
        m.add(
            "ala_pesada",
            cuboid(a, b, -0.42, 0.38, -0.6, 3.5),
            POLICE_DARK,
            0.0,
        );
        let (a, b) = ordered(s * 4.5, s * 5.3);
        m.add("canon", cuboid(a, b, -0.2, 0.3, -3.3, 1.4), LIGHT, 0.0);
        let (a, b) = ordered(s * 2.0, s * 3.35);
        m.add("motor", cuboid(a, b, -0.8, 0.5, 3.5, 5.4), POLICE_DARK, 0.0);
        m.add(
            "motor",
            cuboid(a + 0.12, b - 0.12, -0.55, 0.25, 5.4, 5.5),
            WHITE,
            0.0,
        );
    }
    m
}

fn main() -> io::Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/models");
    fs::create_dir_all(&dir)?;
    for (name, title, model) in [
        ("ship.obj", "nave Tordo alta", ship(true)),
        ("ship_low.obj", "nave Tordo LOD bajo", ship(false)),
        ("tree.obj", "arbol", tree()),
        ("obsidian.obj", "obsidiana", obsidian()),
        ("ice_crystal.obj", "cristal de hielo", ice_crystal()),
        ("building.obj", "monumento pixelado", building()),
        ("police_scout.obj", "patrulla exploradora", police_scout()),
        (
            "police_interceptor.obj",
            "patrulla interceptora",
            police_interceptor(),
        ),
        ("police_heavy.obj", "patrulla pesada", police_heavy()),
    ] {
        model.write(&dir.join(name), title)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ship_keeps_landing_gear_groups_and_lod() {
        let full = ship(true);
        let low = ship(false);
        for name in ["gear_left", "gear_right", "gear_front"] {
            assert!(full.groups.contains(&name));
            assert!(low.groups.contains(&name));
        }
        assert!(full.faces.len() > 1000 && low.faces.len() < full.faces.len());
    }
    #[test]
    fn all_models_have_finite_nonzero_triangles() {
        for m in [
            ship(true),
            ship(false),
            tree(),
            obsidian(),
            ice_crystal(),
            building(),
            police_scout(),
            police_interceptor(),
            police_heavy(),
        ] {
            assert!(!m.faces.is_empty());
            assert!(m.vertices.iter().flatten().all(|v| v.is_finite()));
            for f in m.faces {
                let a = m.vertices[f.v[0]];
                let b = m.vertices[f.v[1]];
                let c = m.vertices[f.v[2]];
                assert!(dot(cross(sub(b, a), sub(c, a)), cross(sub(b, a), sub(c, a))) > 1e-12);
            }
        }
    }
}
