//! Implementación del pipeline: culling -> vértices -> ensamblado/clipping -> tiles -> sombreado diferido.
//!
//! Decisiones:
//! - Los vértices se transforman con `view * model` (espacio de vista) y luego `proj`, para no pasar
//!   por coordenadas de mundo enormes (más precisión con planetas de miles de metros).
//! - Normales: matriz de cofactores de la parte 3x3 del modelo (= det * inversa-transpuesta), con el
//!   signo del determinante, así que soporta escala NO uniforme y espejos.
//! - Rasterizado en punto fijo 28.4 (1/16 de píxel) con aritmética entera exacta: la regla top-left es
//!   exacta y las aristas compartidas nunca dejan huecos ni se pintan dos veces.
//! - Recorte: sólo contra el plano near (z > -w). X/Y se recortan por la caja de la pantalla al
//!   rasterizar (banda de guarda implícita); far sólo hace rechazo trivial (todo el triángulo detrás).
//! - Cada triángulo recortado conserva, por esquina, las baricéntricas del triángulo original; el
//!   buffer de visibilidad guarda baricéntricas del triángulo original ya corregidas por perspectiva.
use super::{DrawCall, FragIn, Framebuffer, RenderOptions, Stats};
use crate::math::{Mat4, Vec3};
use rayon::prelude::*;
use std::time::Instant;

pub(super) const TILE: usize = 64;
const SUB: i32 = 16; // subpíxeles por píxel (4 bits)
const COORD_LIMIT: f64 = (1i64 << 28) as f64; // ±2^24 píxeles en punto fijo
const NONE: u32 = u32::MAX;
const CHUNK: usize = 2048;

/// Información extra de depuración (sólo la usan las pruebas).
#[derive(Default, Clone, Copy, Debug)]
pub(super) struct FrameInfo {
    /// Píxeles cubiertos por algún triángulo antes del z-test (mide el sobredibujo).
    pub raster_hits: u64,
}

struct VOut {
    clip: [f32; 4],
    normal: Vec3,
}

struct CallData {
    mv: Mat4,
    /// `true` si el modelo invierte la orientación (determinante negativo).
    flip: bool,
    verts: Vec<VOut>,
}

#[derive(Clone, Copy)]
struct Tri {
    fx: [i32; 3],
    fy: [i32; 3],
    iw: [f32; 3],
    /// Baricéntricas (respecto al triángulo original) de cada esquina.
    bary: [[f32; 3]; 3],
    call: u32,
    tri: u32,
}

enum Setup {
    Drawn(Tri),
    Back,
    Out,
}

/// Invierte una matriz de vista rígida (rotación + traslación): R^T y -R^T t.
fn invert_rigid(v: &Mat4) -> Mat4 {
    let mut r = Mat4::IDENTITY;
    for i in 0..3 {
        for j in 0..3 {
            r.m[i][j] = v.m[j][i];
        }
    }
    let t = Vec3::new(v.m[0][3], v.m[1][3], v.m[2][3]);
    let it = r.transform_dir(t);
    r.m[0][3] = -it.x;
    r.m[1][3] = -it.y;
    r.m[2][3] = -it.z;
    r
}

/// Planos del frustum en ESPACIO DE VISTA (normal normalizada, adentro = distancia >= 0).
fn frustum_planes(p: &Mat4) -> Vec<[f32; 4]> {
    let r = |i: usize| p.m[i];
    let combos: [(usize, f32); 6] = [(0, 1.0), (0, -1.0), (1, 1.0), (1, -1.0), (2, 1.0), (2, -1.0)];
    let mut out = Vec::with_capacity(6);
    for (row, s) in combos {
        let mut pl = [0.0; 4];
        for k in 0..4 {
            pl[k] = r(3)[k] + s * r(row)[k];
        }
        let n = (pl[0] * pl[0] + pl[1] * pl[1] + pl[2] * pl[2]).sqrt();
        if n > 1e-12 {
            for v in &mut pl {
                *v /= n;
            }
            out.push(pl);
        }
    }
    out
}

fn max_scale(m: &Mat4) -> f32 {
    let mut s: f32 = 0.0;
    for j in 0..3 {
        let c = Vec3::new(m.m[0][j], m.m[1][j], m.m[2][j]);
        s = s.max(c.length());
    }
    s
}

/// Matriz de normales (cofactores del 3x3 del modelo) y signo del determinante.
fn normal_matrix(m: &Mat4) -> ([[f32; 3]; 3], f32) {
    let a = &m.m;
    let (ma, mb, mc) = (a[0][0], a[0][1], a[0][2]);
    let (md, me, mf) = (a[1][0], a[1][1], a[1][2]);
    let (mg, mh, mi) = (a[2][0], a[2][1], a[2][2]);
    let c = [
        [me * mi - mf * mh, mf * mg - md * mi, md * mh - me * mg],
        [mc * mh - mb * mi, ma * mi - mc * mg, mb * mg - ma * mh],
        [mb * mf - mc * me, mc * md - ma * mf, ma * me - mb * md],
    ];
    let det = ma * c[0][0] + mb * c[0][1] + mc * c[0][2];
    (c, if det < 0.0 { -1.0 } else if det > 0.0 { 1.0 } else { 0.0 })
}

fn sphere_visible(planes: &[[f32; 4]], c: Vec3, r: f32) -> bool {
    planes.iter().all(|p| p[0] * c.x + p[1] * c.y + p[2] * c.z + p[3] >= -r)
}

fn outcode(c: &[f32; 4]) -> u8 {
    let (x, y, z, w) = (c[0], c[1], c[2], c[3]);
    (x > w) as u8
        | ((x < -w) as u8) << 1
        | ((y > w) as u8) << 2
        | ((y < -w) as u8) << 3
        | ((z > w) as u8) << 4
        | ((z < -w) as u8) << 5
}

type PolyV = ([f32; 4], [f32; 3]);

/// Sutherland-Hodgman contra z + w >= 0. Devuelve hasta 4 vértices.
fn clip_near(c: &[[f32; 4]; 3]) -> ([PolyV; 4], usize) {
    let e = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut out = [([0.0; 4], [0.0; 3]); 4];
    let mut n = 0;
    for i in 0..3 {
        let j = (i + 1) % 3;
        let (a, b) = (c[i], c[j]);
        let (da, db) = (a[2] + a[3], b[2] + b[3]);
        if da >= 0.0 {
            out[n] = (a, e[i]);
            n += 1;
        }
        if (da >= 0.0) != (db >= 0.0) {
            let t = da / (da - db);
            let mut p = [0.0; 4];
            let mut bc = [0.0; 3];
            for k in 0..4 {
                p[k] = a[k] + (b[k] - a[k]) * t;
            }
            for k in 0..3 {
                bc[k] = e[i][k] + (e[j][k] - e[i][k]) * t;
            }
            out[n] = (p, bc);
            n += 1;
        }
    }
    (out, n)
}

/// Rango de píxeles (inclusive) cuyo centro puede caer dentro del triángulo, recortado a la pantalla.
fn pixel_bounds(t: &Tri, w: usize, h: usize) -> Option<(usize, usize, usize, usize)> {
    let lo = |a: &[i32; 3]| *a.iter().min().unwrap() as i64;
    let hi = |a: &[i32; 3]| *a.iter().max().unwrap() as i64;
    let s = SUB as i64;
    let x0 = (lo(&t.fx) - s / 2 + s - 1).div_euclid(s).max(0);
    let y0 = (lo(&t.fy) - s / 2 + s - 1).div_euclid(s).max(0);
    let x1 = (hi(&t.fx) - s / 2).div_euclid(s).min(w as i64 - 1);
    let y1 = (hi(&t.fy) - s / 2).div_euclid(s).min(h as i64 - 1);
    if x1 < x0 || y1 < y0 {
        None
    } else {
        Some((x0 as usize, y0 as usize, x1 as usize, y1 as usize))
    }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    v: [&PolyV; 3],
    call: u32,
    tri: u32,
    flip: bool,
    cull_back: bool,
    w: usize,
    h: usize,
) -> Setup {
    let mut fx = [0i32; 3];
    let mut fy = [0i32; 3];
    let mut iw = [0f32; 3];
    for i in 0..3 {
        let c = &v[i].0;
        if !(c[3] > 1e-9) || !c[0].is_finite() || !c[1].is_finite() || !c[2].is_finite() || !c[3].is_finite() {
            return Setup::Out;
        }
        let (cw, cx, cy) = (c[3] as f64, c[0] as f64, c[1] as f64);
        let sx = (cx / cw * 0.5 + 0.5) * w as f64;
        let sy = (1.0 - (cy / cw * 0.5 + 0.5)) * h as f64;
        fx[i] = (sx * SUB as f64).round().clamp(-COORD_LIMIT, COORD_LIMIT) as i32;
        fy[i] = (sy * SUB as f64).round().clamp(-COORD_LIMIT, COORD_LIMIT) as i32;
        iw[i] = (1.0 / cw) as f32;
    }
    let area2 = (fx[1] - fx[0]) as i64 * (fy[2] - fy[0]) as i64 - (fy[1] - fy[0]) as i64 * (fx[2] - fx[0]) as i64;
    if area2 == 0 {
        return Setup::Out;
    }
    // En pantalla (y hacia abajo) un triángulo antihorario en NDC tiene área negativa.
    let front = (area2 < 0) != flip;
    if !front && cull_back {
        return Setup::Back;
    }
    let mut bary = [v[0].1, v[1].1, v[2].1];
    if area2 < 0 {
        fx.swap(1, 2);
        fy.swap(1, 2);
        iw.swap(1, 2);
        bary.swap(1, 2);
    }
    let t = Tri { fx, fy, iw, bary, call, tri };
    if pixel_bounds(&t, w, h).is_none() {
        return Setup::Out;
    }
    Setup::Drawn(t)
}

struct TileBuf {
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    depth: Vec<f32>,
    id: Vec<u32>,
    bary: Vec<[f32; 2]>,
    color: Vec<u8>,
    hits: u32,
}

fn raster_tile(tx: usize, ty: usize, fb_w: usize, fb_h: usize, fb_depth: &[f32], tris: &[Tri], list: &[u32]) -> TileBuf {
    let x0 = tx * TILE;
    let y0 = ty * TILE;
    let w = TILE.min(fb_w - x0);
    let h = TILE.min(fb_h - y0);
    let mut depth = Vec::with_capacity(w * h);
    for r in 0..h {
        let s = (y0 + r) * fb_w + x0;
        depth.extend_from_slice(&fb_depth[s..s + w]);
    }
    let mut tb = TileBuf {
        x0,
        y0,
        w,
        h,
        depth,
        id: vec![NONE; w * h],
        bary: vec![[0.0; 2]; w * h],
        color: Vec::new(),
        hits: 0,
    };
    let s = SUB as i64;
    for &tid in list {
        let t = &tris[tid as usize];
        let Some((bx0, by0, bx1, by1)) = pixel_bounds(t, fb_w, fb_h) else { continue };
        let (bx0, by0) = (bx0.max(x0), by0.max(y0));
        let (bx1, by1) = (bx1.min(x0 + w - 1), by1.min(y0 + h - 1));
        if bx1 < bx0 || by1 < by0 {
            continue;
        }
        // Coeficientes de arista: e_i(p) = dx*(py-ay) - dy*(px-ax), i opuesta al vértice i.
        let mut step_x = [0i64; 3];
        let mut step_y = [0i64; 3];
        let mut bias = [0i64; 3];
        let mut base = [0i64; 3];
        let px0 = bx0 as i64 * s + s / 2;
        let py0 = by0 as i64 * s + s / 2;
        for i in 0..3 {
            let a = (i + 1) % 3;
            let b = (i + 2) % 3;
            let (ax, ay) = (t.fx[a] as i64, t.fy[a] as i64);
            let dx = t.fx[b] as i64 - ax;
            let dy = t.fy[b] as i64 - ay;
            step_x[i] = -dy * s;
            step_y[i] = dx * s;
            // top-left: borde superior (dy==0, dx>0) o izquierdo (dy<0) incluyen el borde.
            bias[i] = if (dy == 0 && dx > 0) || dy < 0 { 0 } else { -1 };
            base[i] = dx * (py0 - ay) - dy * (px0 - ax);
        }
        let area2 = (t.fx[1] - t.fx[0]) as i64 * (t.fy[2] - t.fy[0]) as i64
            - (t.fy[1] - t.fy[0]) as i64 * (t.fx[2] - t.fx[0]) as i64;
        let inv_area = 1.0 / area2 as f64;
        let mut row = base;
        for py in by0..=by1 {
            let mut e = row;
            for px in bx0..=bx1 {
                if e[0] + bias[0] >= 0 && e[1] + bias[1] >= 0 && e[2] + bias[2] >= 0 {
                    tb.hits += 1;
                    let l0 = (e[0] as f64 * inv_area) as f32;
                    let l1 = (e[1] as f64 * inv_area) as f32;
                    let l2 = (e[2] as f64 * inv_area) as f32;
                    let q0 = l0 * t.iw[0];
                    let q1 = l1 * t.iw[1];
                    let q2 = l2 * t.iw[2];
                    let d = q0 + q1 + q2;
                    let idx = (py - y0) * w + (px - x0);
                    if d > tb.depth[idx] {
                        tb.depth[idx] = d;
                        tb.id[idx] = tid;
                        let inv = 1.0 / d;
                        let (p0, p1, p2) = (q0 * inv, q1 * inv, q2 * inv);
                        tb.bary[idx] = [
                            p0 * t.bary[0][0] + p1 * t.bary[1][0] + p2 * t.bary[2][0],
                            p0 * t.bary[0][1] + p1 * t.bary[1][1] + p2 * t.bary[2][1],
                        ];
                    }
                }
                for i in 0..3 {
                    e[i] += step_x[i];
                }
            }
            for i in 0..3 {
                row[i] += step_y[i];
            }
        }
    }
    tb
}

fn to_u8(v: f32) -> u8 {
    (v.max(0.0).min(1.0) * 255.0 + 0.5) as u8
}

pub(super) fn render(
    fb: &mut Framebuffer,
    view: Mat4,
    proj: Mat4,
    calls: &[DrawCall],
    opts: &RenderOptions,
    stats: &mut Stats,
) -> FrameInfo {
    let mut info = FrameInfo::default();
    let (w, h) = (fb.width, fb.height);
    stats.draw_calls_total += calls.len() as u32;
    if w == 0 || h == 0 {
        return info;
    }
    let t_vertex = Instant::now();
    let planes = frustum_planes(&proj);
    let inv_view = invert_rigid(&view);

    // (1) frustum culling por esfera + (2) etapa de vértices
    let mut cdata: Vec<CallData> = Vec::with_capacity(calls.len());
    for call in calls {
        let mesh = call.mesh;
        let mv = view * call.model;
        let empty = mesh.indices.len() < 3 || mesh.vertices.is_empty();
        let mut visible = !empty;
        if visible && opts.frustum_culling && mesh.bounds_radius > 0.0 {
            let c = mv.transform_point(mesh.bounds_center);
            visible = sphere_visible(&planes, c, mesh.bounds_radius * max_scale(&call.model));
        }
        if !visible {
            stats.draw_calls_culled += 1;
            cdata.push(CallData { mv, flip: false, verts: Vec::new() });
            continue;
        }
        let (nm, sign) = normal_matrix(&call.model);
        let transform = |v: &crate::geometry::Vertex| {
                let pv = mv.transform_point(v.position);
                let clip = proj.transform_point4(pv);
                let n = v.normal;
                let n = Vec3::new(
                    nm[0][0] * n.x + nm[0][1] * n.y + nm[0][2] * n.z,
                    nm[1][0] * n.x + nm[1][1] * n.y + nm[1][2] * n.z,
                    nm[2][0] * n.x + nm[2][1] * n.y + nm[2][2] * n.z,
                ) * sign;
                VOut { clip, normal: n.normalized() }
            };
        let verts: Vec<VOut> = if mesh.vertices.len() >= 4096 {
            mesh.vertices.par_iter().map(transform).collect()
        } else {
            mesh.vertices.iter().map(transform).collect()
        };
        cdata.push(CallData { mv, flip: sign < 0.0, verts });
    }

    // (3) ensamblado, recorte near, backface
    let mut tris: Vec<Tri> = Vec::new();
    for (ci, call) in calls.iter().enumerate() {
        let cd = &cdata[ci];
        if cd.verts.is_empty() {
            continue;
        }
        let mesh = call.mesh;
        let ntri = mesh.indices.len() / 3;
        stats.triangles_in += ntri as u64;
        let nchunks = ntri.div_ceil(CHUNK);
        let cull_back = opts.backface_culling;
        let process_chunk = |chunk: usize| {
                let mut out = Vec::new();
                let mut cnt = [0u64; 3]; // backface, clipped_out, drawn
                for t in chunk * CHUNK..((chunk + 1) * CHUNK).min(ntri) {
                    let i = [
                        mesh.indices[3 * t] as usize,
                        mesh.indices[3 * t + 1] as usize,
                        mesh.indices[3 * t + 2] as usize,
                    ];
                    let c = [cd.verts[i[0]].clip, cd.verts[i[1]].clip, cd.verts[i[2]].clip];
                    if outcode(&c[0]) & outcode(&c[1]) & outcode(&c[2]) != 0 {
                        cnt[1] += 1;
                        continue;
                    }
                    let (poly, n) = if c.iter().any(|v| v[2] + v[3] < 0.0) {
                        clip_near(&c)
                    } else {
                        let e = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
                        ([(c[0], e[0]), (c[1], e[1]), (c[2], e[2]), ([0.0; 4], [0.0; 3])], 3)
                    };
                    let (mut drawn, mut back) = (false, false);
                    for k in 1..n.saturating_sub(1) {
                        match setup([&poly[0], &poly[k], &poly[k + 1]], ci as u32, t as u32, cd.flip, cull_back, w, h) {
                            Setup::Drawn(tr) => {
                                out.push(tr);
                                drawn = true;
                            }
                            Setup::Back => back = true,
                            Setup::Out => {}
                        }
                    }
                    if drawn {
                        cnt[2] += 1;
                    } else if back {
                        cnt[0] += 1;
                    } else {
                        cnt[1] += 1;
                    }
                }
                (out, cnt)
            };
        let parts: Vec<(Vec<Tri>, [u64; 3])> = if nchunks >= 4 {
            (0..nchunks).into_par_iter().map(process_chunk).collect()
        } else {
            (0..nchunks).map(process_chunk).collect()
        };
        for (v, cnt) in parts {
            stats.triangles_backface += cnt[0];
            stats.triangles_clipped_out += cnt[1];
            stats.triangles_drawn += cnt[2];
            tris.extend(v);
        }
    }
    stats.ms_vertex += t_vertex.elapsed().as_secs_f32() * 1000.0;

    // (4) binning por tiles + rasterizado a buffer de visibilidad
    let t_raster = Instant::now();
    let tiles_x = w.div_ceil(TILE);
    let tiles_y = h.div_ceil(TILE);
    let nt = tiles_x * tiles_y;
    let mut offsets = vec![0u32; nt + 1];
    let tile_range = |t: &Tri| {
        pixel_bounds(t, w, h).map(|(x0, y0, x1, y1)| (x0 / TILE, y0 / TILE, x1 / TILE, y1 / TILE))
    };
    for t in &tris {
        if let Some((a, b, c, d)) = tile_range(t) {
            for ty in b..=d {
                for tx in a..=c {
                    offsets[ty * tiles_x + tx + 1] += 1;
                }
            }
        }
    }
    for i in 0..nt {
        offsets[i + 1] += offsets[i];
    }
    let mut cursor = offsets.clone();
    let mut bins = vec![0u32; offsets[nt] as usize];
    for (id, t) in tris.iter().enumerate() {
        if let Some((a, b, c, d)) = tile_range(t) {
            for ty in b..=d {
                for tx in a..=c {
                    let k = ty * tiles_x + tx;
                    bins[cursor[k] as usize] = id as u32;
                    cursor[k] += 1;
                }
            }
        }
    }
    let fb_depth: &[f32] = &fb.depth;
    let mut tiles: Vec<TileBuf> = (0..nt)
        .into_par_iter()
        .map(|k| {
            let (tx, ty) = (k % tiles_x, k / tiles_x);
            let list = &bins[offsets[k] as usize..offsets[k + 1] as usize];
            raster_tile(tx, ty, w, h, fb_depth, &tris, list)
        })
        .collect();
    info.raster_hits = tiles.iter().map(|t| t.hits as u64).sum();
    stats.ms_raster += t_raster.elapsed().as_secs_f32() * 1000.0;

    // (5) sombreado diferido: sólo píxeles visibles
    let t_shade = Instant::now();
    let (tris_ref, cdata_ref) = (&tris, &cdata);
    let shaded: u64 = tiles
        .par_iter_mut()
        .map(|tile| {
            tile.color = vec![0u8; tile.w * tile.h * 4];
            let mut count = 0u64;
            for idx in 0..tile.w * tile.h {
                let tid = tile.id[idx];
                if tid == NONE {
                    continue;
                }
                let tr = &tris_ref[tid as usize];
                let call = &calls[tr.call as usize];
                let cd = &cdata_ref[tr.call as usize];
                let mesh = call.mesh;
                let t = tr.tri as usize;
                let ix = [
                    mesh.indices[3 * t] as usize,
                    mesh.indices[3 * t + 1] as usize,
                    mesh.indices[3 * t + 2] as usize,
                ];
                let vs = [&mesh.vertices[ix[0]], &mesh.vertices[ix[1]], &mesh.vertices[ix[2]]];
                let [b0, b1] = tile.bary[idx];
                let b2 = 1.0 - b0 - b1;
                let p_local = vs[0].position * b0 + vs[1].position * b1 + vs[2].position * b2;
                let normal = (cd.verts[ix[0]].normal * b0 + cd.verts[ix[1]].normal * b1 + cd.verts[ix[2]].normal * b2)
                    .normalized();
                let uv = [
                    vs[0].uv[0] * b0 + vs[1].uv[0] * b1 + vs[2].uv[0] * b2,
                    vs[0].uv[1] * b0 + vs[1].uv[1] * b1 + vs[2].uv[1] * b2,
                ];
                let mut color = [0.0; 3];
                for k in 0..3 {
                    color[k] = vs[0].color[k] * b0 + vs[1].color[k] * b1 + vs[2].color[k] * b2;
                }
                let pv = cd.mv.transform_point(p_local);
                let frag = FragIn {
                    p_local,
                    normal,
                    uv,
                    color,
                    view_dir: inv_view.transform_dir(-pv).normalized(),
                    distance: pv.length(),
                };
                let rgb = call.shader.shade(&frag, &call.uniforms);
                let o = idx * 4;
                tile.color[o] = to_u8(rgb[0]);
                tile.color[o + 1] = to_u8(rgb[1]);
                tile.color[o + 2] = to_u8(rgb[2]);
                tile.color[o + 3] = 255;
                count += 1;
            }
            count
        })
        .sum();
    stats.pixels_shaded += shaded;
    // Copia de cada tile al framebuffer (sólo píxeles ganadores).
    for tile in &tiles {
        for r in 0..tile.h {
            let fbrow = (tile.y0 + r) * w + tile.x0;
            for c in 0..tile.w {
                let idx = r * tile.w + c;
                if tile.id[idx] != NONE {
                    let o = (fbrow + c) * 4;
                    fb.color[o..o + 4].copy_from_slice(&tile.color[idx * 4..idx * 4 + 4]);
                    fb.depth[fbrow + c] = tile.depth[idx];
                }
            }
        }
    }
    stats.ms_shade += t_shade.elapsed().as_secs_f32() * 1000.0;
    info
}
