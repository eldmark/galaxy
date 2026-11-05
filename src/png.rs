//! Escritor PNG mínimo (RGBA 8 bits, DEFLATE sin compresión). Sin dependencias.
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut table = [0u32; 256];
    for (n, t) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 == 1 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *t = c;
    }
    let mut crc = !0_u32;
    for part in parts {
        for &b in *part {
            crc = table[((crc ^ b as u32) & 0xff) as usize] ^ (crc >> 8);
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1_u32, 0_u32);
    for block in bytes.chunks(5552) {
        for &byte in block {
            a += byte as u32;
            b += a;
        }
        a %= 65_521;
        b %= 65_521;
    }
    (b << 16) | a
}

fn chunk<W: Write>(out: &mut W, name: &[u8; 4], data: &[u8]) -> io::Result<()> {
    out.write_all(&(data.len() as u32).to_be_bytes())?;
    out.write_all(name)?;
    out.write_all(data)?;
    out.write_all(&crc32(&[name, data]).to_be_bytes())
}

/// Guarda `rgba` (width*height*4 bytes) como PNG.
pub fn save(path: &Path, width: usize, height: usize, rgba: &[u8]) -> io::Result<()> {
    if rgba.len() != width * height * 4 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "tamaño de buffer incorrecto"));
    }
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all(b"\x89PNG\r\n\x1a\n")?;
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header)?;

    let mut raw = Vec::with_capacity(height * (width * 4 + 1));
    for row in rgba.chunks_exact((width * 4).max(1)) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    let mut zlib = vec![0x78, 0x01];
    let mut remaining = raw.as_slice();
    loop {
        let len = remaining.len().min(65_535);
        let last = len == remaining.len();
        zlib.push(last as u8);
        zlib.extend_from_slice(&(len as u16).to_le_bytes());
        zlib.extend_from_slice(&(!(len as u16)).to_le_bytes());
        zlib.extend_from_slice(&remaining[..len]);
        remaining = &remaining[len..];
        if last {
            break;
        }
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
    chunk(&mut out, b"IDAT", &zlib)?;
    chunk(&mut out, b"IEND", &[])?;
    out.flush()
}
