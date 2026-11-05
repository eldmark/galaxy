//! Cargador de OBJ.
//!
//! Soporta `v x y z [r g b]`, `vt u v [w]`, `vn x y z`, `f` con `v`, `v/vt`, `v//vn`, `v/vt/vn`,
//! índices negativos (relativos al final), polígonos triangulados en abanico, comentarios (`#`) y
//! líneas vacías. Otras directivas (`o`, `g`, `s`, `usemtl`, `mtllib`) se ignoran.
//! Un vértice de la malla es una combinación única `(v, vt, vn)`. Sin `vt`: uv = (0,0). Sin color:
//! blanco. Si algún vértice de cara no tiene `vn`, se calculan normales suaves (soldadas por
//! posición) y se usan solo para esos vértices.
use super::{Mesh, Vertex, normals};
use crate::math::Vec3;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
pub struct ObjError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ObjError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line > 0 {
            write!(f, "OBJ inválido (línea {}): {}", self.line, self.message)
        } else {
            write!(f, "OBJ inválido: {}", self.message)
        }
    }
}

impl std::error::Error for ObjError {}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, ObjError> {
    Err(ObjError { line, message: message.into() })
}

/// Carga un OBJ desde disco (ver [`parse_obj`]).
pub fn load_obj(path: &str) -> Result<Mesh, Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("no se pudo leer '{path}': {e}"))?;
    Ok(parse_obj(&text)?)
}

fn floats(parts: &[&str], line: usize, what: &str) -> Result<Vec<f32>, ObjError> {
    let mut out = Vec::with_capacity(parts.len());
    for p in parts {
        match p.parse::<f32>() {
            Ok(x) if x.is_finite() => out.push(x),
            _ => return err(line, format!("número inválido '{p}' en {what}")),
        }
    }
    Ok(out)
}

fn resolve(tok: &str, len: usize, line: usize, what: &str) -> Result<usize, ObjError> {
    let i: i64 = tok.parse().or_else(|_| err(line, format!("índice inválido '{tok}' de {what}")))?;
    let r = if i > 0 { i - 1 } else if i < 0 { len as i64 + i } else { -1 };
    if r < 0 || r >= len as i64 {
        return err(line, format!("índice {i} de {what} fuera de rango (hay {len})"));
    }
    Ok(r as usize)
}

/// Interpreta el texto de un OBJ. Nunca entra en pánico: los errores salen como `Err`.
pub fn parse_obj(text: &str) -> Result<Mesh, ObjError> {
    let mut pos: Vec<Vec3> = Vec::new();
    let mut col: Vec<[f32; 3]> = Vec::new();
    let mut uvs: Vec<[f32; 2]> = Vec::new();
    let mut nrm: Vec<Vec3> = Vec::new();
    let mut mesh = Mesh::default();
    let mut has_n: Vec<bool> = Vec::new();
    let mut cache: HashMap<(usize, Option<usize>, Option<usize>), u32> = HashMap::new();

    for (ln, raw) in text.lines().enumerate() {
        let line = ln + 1;
        let s = raw.split('#').next().unwrap_or("").trim();
        if s.is_empty() {
            continue;
        }
        let parts: Vec<&str> = s.split_whitespace().collect();
        match parts[0] {
            "v" => {
                let a = &parts[1..];
                if a.len() != 3 && a.len() != 6 && a.len() != 4 {
                    return err(line, "'v' necesita 3 coordenadas (opcional 3 de color)");
                }
                let f = floats(a, line, "v")?;
                pos.push(Vec3::new(f[0], f[1], f[2]));
                col.push(if a.len() == 6 {
                    [f[3].clamp(0.0, 1.0), f[4].clamp(0.0, 1.0), f[5].clamp(0.0, 1.0)]
                } else {
                    [1.0, 1.0, 1.0]
                });
            }
            "vt" => {
                if parts.len() < 3 || parts.len() > 4 {
                    return err(line, "'vt' necesita 2 o 3 valores");
                }
                let f = floats(&parts[1..], line, "vt")?;
                uvs.push([f[0], f[1]]);
            }
            "vn" => {
                if parts.len() != 4 {
                    return err(line, "'vn' necesita 3 valores");
                }
                let f = floats(&parts[1..], line, "vn")?;
                nrm.push(Vec3::new(f[0], f[1], f[2]).normalized());
            }
            "f" => {
                if parts.len() < 4 {
                    return err(line, "'f' necesita al menos 3 vértices");
                }
                let mut face: Vec<u32> = Vec::with_capacity(parts.len() - 1);
                for tok in &parts[1..] {
                    let fields: Vec<&str> = tok.split('/').collect();
                    if fields.len() > 3 || fields[0].is_empty() {
                        return err(line, format!("vértice de cara inválido '{tok}'"));
                    }
                    let vi = resolve(fields[0], pos.len(), line, "v")?;
                    let ti = match fields.get(1) {
                        Some(t) if !t.is_empty() => Some(resolve(t, uvs.len(), line, "vt")?),
                        _ => None,
                    };
                    let ni = match fields.get(2) {
                        Some(t) if !t.is_empty() => Some(resolve(t, nrm.len(), line, "vn")?),
                        _ => None,
                    };
                    let idx = match cache.get(&(vi, ti, ni)) {
                        Some(&i) => i,
                        None => {
                            let i = mesh.vertices.len() as u32;
                            mesh.vertices.push(Vertex {
                                position: pos[vi],
                                normal: ni.map(|n| nrm[n]).unwrap_or(Vec3::ZERO),
                                uv: ti.map(|t| uvs[t]).unwrap_or([0.0, 0.0]),
                                color: col[vi],
                            });
                            has_n.push(ni.is_some());
                            cache.insert((vi, ti, ni), i);
                            i
                        }
                    };
                    face.push(idx);
                }
                for k in 1..face.len() - 1 {
                    mesh.indices.extend_from_slice(&[face[0], face[k], face[k + 1]]);
                }
            }
            _ => {}
        }
    }
    if mesh.indices.is_empty() {
        return err(0, "el archivo no contiene caras ('f')");
    }
    if has_n.iter().any(|h| !h) {
        let smooth = normals::smooth_normals(&mesh);
        for (i, v) in mesh.vertices.iter_mut().enumerate() {
            if !has_n[i] {
                v.normal = if smooth[i] == Vec3::ZERO { Vec3::Y } else { smooth[i] };
            }
        }
    }
    mesh.update_bounds();
    Ok(mesh)
}
