//! Carga de OBJ dividido por grupos (`o` / `g`), p. ej. para animar partes de la nave.
use super::Mesh;
use super::obj::{ObjError, parse_obj};

/// Divide un OBJ por líneas `o`/`g`. Cada grupo con caras da una malla con sus propios vértices
/// (reindexados), colores y bounds. Las caras previas al primer `o`/`g` van al grupo "default".
/// Los grupos sin caras se omiten. Mantiene el orden de aparición; un nombre repetido se fusiona.
pub fn parse_obj_groups(text: &str) -> Result<Vec<(String, Mesh)>, ObjError> {
    // Grupo de cada línea `f`; el resto de líneas (v, vt, vn) se conservan en orden para cada
    // grupo, así los índices (incluso los negativos) siguen resolviéndose igual.
    let mut names: Vec<String> = Vec::new();
    let mut face_group: Vec<Option<usize>> = Vec::new();
    let mut cur: Option<usize> = None;
    for raw in text.lines() {
        let s = raw.split('#').next().unwrap_or("").trim();
        let mut it = s.split_whitespace();
        match it.next() {
            Some("o") | Some("g") => {
                let name = it.next().unwrap_or("default").to_string();
                cur = Some(names.iter().position(|n| *n == name).unwrap_or_else(|| {
                    names.push(name);
                    names.len() - 1
                }));
                face_group.push(None);
            }
            Some("f") => {
                if cur.is_none() {
                    names.push("default".to_string());
                    cur = Some(names.len() - 1);
                }
                face_group.push(cur);
            }
            _ => face_group.push(None),
        }
    }
    let mut out = Vec::new();
    for (gi, name) in names.iter().enumerate() {
        // Un grupo sin caras haría fallar parse_obj: se omite.
        if !face_group.iter().any(|g| *g == Some(gi)) {
            continue;
        }
        let mut sub = String::with_capacity(text.len() / 2);
        for (raw, fg) in text.lines().zip(face_group.iter()) {
            let t = raw.trim_start();
            let is_face = t.starts_with("f ") || t.starts_with("f\t");
            if is_face && *fg != Some(gi) {
                continue;
            }
            sub.push_str(raw);
            sub.push('\n');
        }
        out.push((name.clone(), parse_obj(&sub)?));
    }
    Ok(out)
}

/// Carga un OBJ desde disco y lo divide por grupos (ver [`parse_obj_groups`]).
pub fn load_obj_groups(path: &str) -> Result<Vec<(String, Mesh)>, ObjError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| ObjError { line: 0, message: format!("no se pudo leer '{path}': {e}") })?;
    parse_obj_groups(&text)
}
