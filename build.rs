//! Embeds the icon (the same as the window and tray icon) into the .exe so Explorer and shortcuts show it.

use std::io::Write;

#[allow(dead_code)]
#[path = "src/icon.rs"]
mod icon;

/// PNG from RGBA (Windows Vista+ accepts PNG inside .ico at all sizes).
fn png(size: u32, rgba: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut crc = flate2::Crc::new();
        crc.update(kind);
        crc.update(data);
        out.extend(kind);
        out.extend(data);
        out.extend(crc.sum().to_be_bytes());
    }
    let mut raw = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks(size as usize * 4) {
        raw.push(0);
        raw.extend(row);
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&raw).unwrap();
    let mut ihdr = Vec::new();
    ihdr.extend(size.to_be_bytes());
    ihdr.extend(size.to_be_bytes());
    ihdr.extend([8, 6, 0, 0, 0]);
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z.finish().unwrap());
    chunk(&mut out, b"IEND", &[]);
    out
}

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let sizes = [16u32, 20, 24, 32, 40, 48, 64, 256];
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| png(s, &icon::icon_rgba(s))).collect();

    let mut ico = Vec::new();
    ico.extend([0, 0, 1, 0]);
    ico.extend((sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (s, img) in sizes.iter().zip(&images) {
        let dim = if *s >= 256 { 0 } else { *s as u8 };
        ico.extend([dim, dim, 0, 0]);
        ico.extend(1u16.to_le_bytes());
        ico.extend(32u16.to_le_bytes());
        ico.extend((img.len() as u32).to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in &images {
        ico.extend(img);
    }
    std::fs::write(out.join("infinifan.ico"), &ico).unwrap();
    let rc = out.join("infinifan.rc");
    let ico_path = out.join("infinifan.ico").display().to_string().replace('\\', "/");
    std::fs::write(&rc, format!("1 ICON \"{ico_path}\"\n")).unwrap();
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().unwrap();
}
